use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn provider_redirect_cannot_forward_credentials_to_another_endpoint() {
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_url = format!("http://{}", source.local_addr().unwrap());
    let redirect = format!("http://{}/v1/models", destination.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut stream, _) = source.accept().await.unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 1024];
        while !request.windows(4).any(|part| part == b"\r\n\r\n") {
            let read = stream.read(&mut bytes).await.unwrap();
            assert_ne!(read, 0);
            request.extend_from_slice(&bytes[..read]);
        }
        assert!(
            String::from_utf8(request)
                .unwrap()
                .to_ascii_lowercase()
                .contains("x-api-key: fixture-credential")
        );
        let body = r#"{"type":"error","error":{"type":"redirect","message":"Use the canonical endpoint"}}"#;
        stream.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {redirect}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    let client = RigProviderClient::new(
        &crate::tests::provider_config(LanguageModelProtocol::Anthropic, source_url),
        Some(&SecretCredential::new("fixture-credential").unwrap()),
    )
    .unwrap();
    let result = tokio::select! {
        _ = destination.accept() => panic!("a provider redirect attempted to contact another endpoint"),
        result = tokio::time::timeout(Duration::from_secs(5), client.list_models()) => result.unwrap(),
    };
    assert_eq!(
        result.unwrap_err().code,
        AgentDriverFailureCode::ProviderRequestRejected
    );
    server.await.unwrap();
}
