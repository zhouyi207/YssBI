use super::*;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Default)]
struct Delegation {
    tasks: Mutex<Vec<model::AgentTaskInput>>,
}

impl ModelCapabilityExecutor for Delegation {
    fn begin_control<'a>(
        &'a self,
        tool: AgentControlTool,
    ) -> AgentFuture<'a, Result<ToolInvocationId, CapabilityFailure>> {
        StaticExecutor.begin_control(tool)
    }

    fn finish_control<'a>(
        &'a self,
        id: ToolInvocationId,
        tool: AgentControlTool,
        failure: Option<CapabilityFailure>,
    ) -> AgentFuture<'a, Result<(), CapabilityFailure>> {
        StaticExecutor.finish_control(id, tool, failure)
    }

    fn execute<'a>(
        &'a self,
        _: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async { panic!("delegation must not invoke business tools") })
    }

    fn delegate<'a>(
        &'a self,
        task: model::AgentTaskInput,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async move {
            let role = task.worker;
            self.tasks.lock().unwrap().push(task);
            Ok(AgentTaskOutcome {
                run_id: AgentRunId::try_new("review-run").unwrap(),
                role,
                state: AgentRunState::Completed,
                report: None,
                failure_code: None,
                artifacts: vec![],
                results: vec![],
                evidence: vec![],
                plan: None,
                invalidated_runs: vec![],
            })
        })
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Value {
    let mut received = Vec::new();
    let mut buffer = [0; 4096];
    let (header_end, length) = loop {
        let count = stream.read(&mut buffer).await.unwrap();
        assert!(count > 0, "incomplete request headers");
        received.extend_from_slice(&buffer[..count]);
        if let Some(end) = received.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&received[..end]).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            break (end + 4, length);
        }
    };
    while received.len() < header_end + length {
        let count = stream.read(&mut buffer).await.unwrap();
        assert!(count > 0, "incomplete request body");
        received.extend_from_slice(&buffer[..count]);
    }
    serde_json::from_slice(&received[header_end..header_end + length]).unwrap()
}

fn delegation_stream(arguments: &Value, id: usize) -> String {
    let start = json!({"id":format!("chat-{id}"),"choices":[{"index":0,"delta":{
        "role":"assistant", "tool_calls":[{"index":0,"id":format!("call-{id}"),
        "type":"function","function":{"name":"delegate_task","arguments":""}}]
    },"finish_reason":null}]});
    let mut events = format!("data: {start}\n\n");
    // Split inside field names, escaped strings and non-ASCII content.
    let characters = arguments.to_string().chars().collect::<Vec<_>>();
    for chunk in characters.chunks(7) {
        let frame = json!({"id":format!("chat-{id}"),"choices":[{"index":0,"delta":{
            "tool_calls":[{"index":0,"function":{"arguments":chunk.iter().collect::<String>()}}]
        },"finish_reason":null}]});
        events.push_str(&format!("data: {frame}\n\n"));
    }
    let finish = json!({"id":format!("chat-{id}"),"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]});
    events.push_str(&format!("data: {finish}\n\ndata: [DONE]\n\n"));
    events
}

#[tokio::test]
async fn native_deepseek_preserves_fragmented_delegation_and_accepts_corrected_retry() {
    let task = model::AgentTaskInput {
        worker: AgentRole::Review,
        objective: "检查 diamonds 报告".into(),
        constraints: "只读；保留 \"cut\" 字段\n禁止修改原数据".into(),
        completion_criteria: "返回核验结论".into(),
        depends_on: vec![],
        scope: model::AgentTaskScopeInput::default(),
    };
    let valid = serde_json::to_value(&task).unwrap();
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("constraints");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut config = provider_config(
        LanguageModelProtocol::OpenAiChat,
        format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    config.adapter = "deepseek/openai".into();
    let driver = configured_driver(config);
    let executor = Arc::new(Delegation::default());
    let mut input = request(vec![]);
    input.role = AgentRole::Manager;
    input.control_tools = vec![AgentControlTool::DelegateTask];
    let server = async {
        for (index, response) in [
            delegation_stream(&missing, 0),
            delegation_stream(&valid, 1),
            provider_stream(LanguageModelProtocol::OpenAiChat),
        ]
        .into_iter()
        .enumerate()
        {
            let (mut stream, _) = listener.accept().await.unwrap();
            let body = read_request(&mut stream).await;
            let schema = &body["tools"][0]["function"]["parameters"];
            assert_eq!(schema["properties"]["constraints"]["type"], "string");
            assert!(
                schema["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("constraints"))
            );
            if index == 1 {
                let feedback = body["messages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rev()
                    .find(|message| message["role"] == "tool")
                    .unwrap();
                let content = feedback["content"].as_str().unwrap();
                assert!(content.contains("missing_field"), "{content}");
                assert!(content.contains("$.constraints"), "{content}");
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            stream.write_all(headers.as_bytes()).await.unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        }
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        let (_, result) = tokio::join!(
            server,
            driver.run_turn(
                input,
                executor.clone(),
                Arc::new(CollectingOutput::default()),
                CancellationToken::default(),
            )
        );
        result.unwrap();
    })
    .await
    .unwrap();
    assert_eq!(*executor.tasks.lock().unwrap(), vec![task]);
}
