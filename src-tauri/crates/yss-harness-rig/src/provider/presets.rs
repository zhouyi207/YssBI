//! Connection templates. Rig remains the owner of native dialect defaults.
use crate::RigProviderConfigurationError;
use rig_core::providers::registry::{ProviderConfig, ProviderId};
use yss_harness_contract::{
    LanguageModelAuthentication as Auth, LanguageModelProtocol as Protocol,
    LanguageModelProviderPreset,
};

pub fn provider_presets() -> Result<Vec<LanguageModelProviderPreset>, RigProviderConfigurationError>
{
    let native = [
        (
            "openai",
            "OpenAI",
            "openai/openai",
            Protocol::OpenAiResponses,
        ),
        (
            "anthropic",
            "Anthropic / Claude",
            "anthropic/anthropic",
            Protocol::Anthropic,
        ),
        (
            "gemini",
            "Google / Gemini",
            "gcp.gemini/gemini",
            Protocol::Gemini,
        ),
        (
            "deepseek",
            "DeepSeek",
            "deepseek/openai",
            Protocol::OpenAiChat,
        ),
        (
            "moonshot",
            "Moonshot / Kimi",
            "moonshot/openai",
            Protocol::OpenAiChat,
        ),
        ("zai", "Z.AI / GLM", "zai/openai", Protocol::OpenAiChat),
        ("minimax", "MiniMax", "minimax/openai", Protocol::OpenAiChat),
        (
            "xiaomimimo",
            "Xiaomi / MiMo",
            "xiaomimimo/openai",
            Protocol::OpenAiChat,
        ),
        ("xai", "xAI / Grok", "xai/openai", Protocol::OpenAiResponses),
        (
            "openrouter",
            "OpenRouter",
            "openrouter/openai",
            Protocol::OpenAiChat,
        ),
        ("groq", "Groq", "groq/openai", Protocol::OpenAiChat),
        ("mistral", "Mistral", "mistral/openai", Protocol::OpenAiChat),
        (
            "together",
            "Together AI",
            "together/openai",
            Protocol::OpenAiChat,
        ),
        (
            "huggingface",
            "Hugging Face",
            "huggingface/openai",
            Protocol::OpenAiChat,
        ),
        (
            "hyperbolic",
            "Hyperbolic",
            "hyperbolic/openai",
            Protocol::OpenAiChat,
        ),
    ];
    let mut presets = native
        .into_iter()
        .map(|(id, name, adapter, protocol)| {
            let config = ProviderId::resolve(adapter)
                .map_err(|_| RigProviderConfigurationError::Invalid)?
                .config("");
            let base_url = match config {
                ProviderConfig::OpenAi(config) => config.base_url,
                ProviderConfig::Anthropic(config) => config.base_url,
                ProviderConfig::Gemini(config) => config.base_url,
            };
            Ok(template(
                id,
                name,
                adapter,
                protocol,
                Auth::ApiKey,
                &base_url,
            ))
        })
        .collect::<Result<Vec<_>, RigProviderConfigurationError>>()?;

    // Regional and compatible services have an explicit endpoint, while using
    // the same Rig wire implementation as their advertised API.
    for (id, name, adapter, url) in [
        (
            "moonshot-cn",
            "Kimi · 中国",
            "moonshot/openai",
            "https://api.moonshot.cn/v1",
        ),
        (
            "minimax-cn",
            "MiniMax · 中国",
            "minimax/openai",
            "https://api.minimaxi.com/v1",
        ),
        (
            "zhipu",
            "智谱 / GLM · 中国",
            "zai/openai",
            "https://open.bigmodel.cn/api/paas/v4",
        ),
        (
            "cerebras",
            "Cerebras",
            "openai/openai",
            "https://api.cerebras.ai/v1",
        ),
        (
            "nvidia",
            "NVIDIA NIM",
            "openai/openai",
            "https://integrate.api.nvidia.com/v1",
        ),
        (
            "dashscope",
            "阿里云百炼 / Qwen · 北京",
            "openai/openai",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        ),
        (
            "dashscope-intl",
            "Alibaba Cloud / Qwen · Singapore",
            "openai/openai",
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        ),
        (
            "dashscope-us",
            "Alibaba Cloud / Qwen · US",
            "openai/openai",
            "https://dashscope-us.aliyuncs.com/compatible-mode/v1",
        ),
        (
            "doubao",
            "火山方舟 / Doubao",
            "openai/openai",
            "https://ark.cn-beijing.volces.com/api/v3",
        ),
        (
            "siliconflow",
            "硅基流动 / SiliconFlow",
            "openai/openai",
            "https://api.siliconflow.cn/v1",
        ),
        (
            "stepfun",
            "阶跃星辰 / StepFun",
            "openai/openai",
            "https://api.stepfun.com/v1",
        ),
    ] {
        presets.push(template(
            id,
            name,
            adapter,
            Protocol::OpenAiChat,
            Auth::ApiKey,
            url,
        ));
    }
    presets.push(template(
        "azure",
        "Azure OpenAI v1",
        "openai/openai",
        Protocol::OpenAiResponses,
        Auth::ApiKey,
        "",
    ));
    for (id, name, adapter, url) in [
        (
            "ollama",
            "Ollama",
            "openai/openai",
            "http://localhost:11434/v1",
        ),
        (
            "lm-studio",
            "LM Studio",
            "openai/openai",
            "http://localhost:1234/v1",
        ),
        (
            "llamacpp",
            "llama.cpp",
            "llamacpp/openai",
            "http://localhost:8080/v1",
        ),
        ("vllm", "vLLM", "openai/openai", "http://localhost:8000/v1"),
    ] {
        presets.push(template(
            id,
            name,
            adapter,
            Protocol::OpenAiChat,
            Auth::None,
            url,
        ));
    }
    for (id, name, adapter, protocol) in [
        (
            "custom-chat",
            "Custom · OpenAI Chat Completions",
            "openai/openai",
            Protocol::OpenAiChat,
        ),
        (
            "custom-responses",
            "Custom · OpenAI Responses",
            "openai/openai",
            Protocol::OpenAiResponses,
        ),
        (
            "custom-anthropic",
            "Custom · Anthropic Messages",
            "anthropic/anthropic",
            Protocol::Anthropic,
        ),
        (
            "custom-gemini",
            "Custom · Gemini Interactions",
            "gcp.gemini/gemini",
            Protocol::Gemini,
        ),
    ] {
        presets.push(template(id, name, adapter, protocol, Auth::ApiKey, ""));
    }
    Ok(presets)
}

fn template(
    id: &str,
    name: &str,
    adapter: &str,
    protocol: Protocol,
    authentication: Auth,
    base_url: &str,
) -> LanguageModelProviderPreset {
    LanguageModelProviderPreset {
        id: id.into(),
        name: name.into(),
        protocol,
        adapter: adapter.into(),
        authentication,
        base_url: base_url.into(),
    }
}
