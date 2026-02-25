use crate::ipc::IpcClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// 诊断结果
#[derive(Debug, Clone)]
pub struct Diagnosis {
    pub pid: u32,
    pub causes: Vec<String>,
    pub recommendation: String,
    pub confidence: f64,
}

/// 大模型提供商
#[derive(Debug, Clone)]
pub enum LlmProvider {
    OpenAI,
    Claude,
    Local, // 本地模型（未来支持）
}

impl LlmProvider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" | "gpt" => LlmProvider::OpenAI,
            "claude" | "anthropic" => LlmProvider::Claude,
            _ => LlmProvider::OpenAI, // 默认
        }
    }

    pub fn api_url(&self) -> &str {
        match self {
            LlmProvider::OpenAI => "https://api.openai.com/v1/chat/completions",
            LlmProvider::Claude => "https://api.anthropic.com/v1/messages",
            LlmProvider::Local => "http://localhost:11434/api/generate", // Ollama 默认端口
        }
    }
}

/// 大模型客户端
pub struct LlmClient {
    provider: LlmProvider,
    api_key: String,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(provider: LlmProvider, api_key: String) -> Self {
        Self {
            provider,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// 从环境变量创建客户端
    pub fn from_env() -> Result<Self, String> {
        let provider = std::env::var("XCTL_LLM_PROVIDER")
            .map(|s| LlmProvider::from_str(&s))
            .unwrap_or(LlmProvider::OpenAI);

        let api_key = match provider {
            LlmProvider::OpenAI => {
                std::env::var("OPENAI_API_KEY")
                    .or_else(|_| std::env::var("XCTL_OPENAI_API_KEY"))
                    .map_err(|_| "未设置 OPENAI_API_KEY 环境变量".to_string())?
            }
            LlmProvider::Claude => {
                std::env::var("ANTHROPIC_API_KEY")
                    .or_else(|_| std::env::var("XCTL_ANTHROPIC_API_KEY"))
                    .map_err(|_| "未设置 ANTHROPIC_API_KEY 环境变量".to_string())?
            }
            LlmProvider::Local => "".to_string(), // 本地模型不需要 API key
        };

        Ok(Self::new(provider, api_key))
    }

    /// 调用大模型获取诊断建议
    pub async fn diagnose(
        &self,
        pid: u32,
        causes: Vec<String>,
        processes: Vec<serde_json::Value>,
    ) -> Result<Diagnosis, String> {
        let prompt = build_diagnosis_prompt(pid, causes, processes);

        let response = match self.provider {
            LlmProvider::OpenAI => self.call_openai(&prompt).await?,
            LlmProvider::Claude => self.call_claude(&prompt).await?,
            LlmProvider::Local => self.call_local(&prompt).await?,
        };

        parse_diagnosis_response(response)
    }

    async fn call_openai(&self, prompt: &str) -> Result<String, String> {
        let body = json!({
            "model": "gpt-4o-mini", // 使用成本更低的模型
            "messages": [
                {
                    "role": "system",
                    "content": "你是一位资深的 SRE（Site Reliability Engineer），专门诊断 AI 基础设施的性能问题。请用简洁、专业的中文回答。"
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.3,
            "max_tokens": 500
        });

        let response = self
            .client
            .post(self.provider.api_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("API 请求失败: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API 错误: {} - {}", response.status(), error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| "响应格式错误".to_string())
            .map(|s| s.to_string())
    }

    async fn call_claude(&self, prompt: &str) -> Result<String, String> {
        let body = json!({
            "model": "claude-3-haiku-20240307", // 使用成本更低的模型
            "max_tokens": 500,
            "messages": [
                {
                    "role": "user",
                    "content": format!("你是一位资深的 SRE（Site Reliability Engineer），专门诊断 AI 基础设施的性能问题。请用简洁、专业的中文回答。\n\n{}", prompt)
                }
            ]
        });

        let response = self
            .client
            .post(self.provider.api_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("API 请求失败: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API 错误: {} - {}", response.status(), error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| "响应格式错误".to_string())
            .map(|s| s.to_string())
    }

    async fn call_local(&self, prompt: &str) -> Result<String, String> {
        // 本地模型（如 Ollama）的调用
        let body = json!({
            "model": "llama2", // 默认模型，可通过环境变量配置
            "prompt": format!("你是一位资深的 SRE。请用简洁、专业的中文回答。\n\n{}", prompt),
            "stream": false
        });

        let response = self
            .client
            .post(self.provider.api_url())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("API 请求失败: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("API 错误: {} - {}", response.status(), error_text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("解析响应失败: {}", e))?;

        json["response"]
            .as_str()
            .ok_or_else(|| "响应格式错误".to_string())
            .map(|s| s.to_string())
    }
}

/// 构建诊断 Prompt
fn build_diagnosis_prompt(
    pid: u32,
    causes: Vec<String>,
    processes: Vec<serde_json::Value>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "## 问题描述\n\n进程 PID {} 出现性能问题。\n\n",
        pid
    ));

    if !causes.is_empty() {
        prompt.push_str("## 阻塞根因分析\n\n");
        for (idx, cause) in causes.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", idx + 1, cause));
        }
        prompt.push_str("\n");
    }

    // 添加相关进程信息
    if !processes.is_empty() {
        prompt.push_str("## 相关进程信息\n\n");
        for proc in processes.iter().take(5) {
            // 只显示前 5 个相关进程
            let proc_pid = proc["pid"].as_u64().unwrap_or(0);
            let state = proc["state"].as_str().unwrap_or("unknown");
            let resources = proc["resources"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            prompt.push_str(&format!(
                "- PID {}: 状态={}, 资源=[{}]\n",
                proc_pid, state, resources
            ));
        }
        prompt.push_str("\n");
    }

    prompt.push_str(
        "## 请提供诊断建议\n\n\
        请基于以上信息，提供：\n\
        1. 问题根因分析（用一句话概括）\n\
        2. 具体的修复建议（3-5 条）\n\
        3. 预防措施（可选）\n\n\
        请用简洁、专业的中文回答，避免技术术语过多。",
    );

    prompt
}

/// 解析大模型响应
fn parse_diagnosis_response(response: String) -> Result<Diagnosis, String> {
    // 简化处理：直接使用响应作为建议
    // 未来可以解析结构化响应（JSON 格式）
    Ok(Diagnosis {
        pid: 0, // 会在调用处设置
        causes: vec![],
        recommendation: response.trim().to_string(),
        confidence: 0.8, // 默认置信度
    })
}

/// 执行诊断
pub async fn run_diagnosis(
    pid: u32,
    port: u16,
    llm_provider: Option<String>,
) -> Result<Diagnosis, Box<dyn std::error::Error>> {
    // 连接到 daemon
    let client = IpcClient::new(port);

    if !client.ping().await? {
        return Err("无法连接到 daemon，请先运行: xctl run".into());
    }

    // 获取阻塞根因
    let causes = client.why_process(pid).await?;

    // 获取进程列表（用于上下文）
    let processes = client.list_processes().await?;

    // 创建 LLM 客户端
    let llm_client = if let Some(provider_str) = llm_provider {
        let provider = LlmProvider::from_str(&provider_str);
        let api_key = match provider {
            LlmProvider::OpenAI => {
                std::env::var("OPENAI_API_KEY")
                    .or_else(|_| std::env::var("XCTL_OPENAI_API_KEY"))
                    .map_err(|_| "未设置 OPENAI_API_KEY 环境变量")?
            }
            LlmProvider::Claude => {
                std::env::var("ANTHROPIC_API_KEY")
                    .or_else(|_| std::env::var("XCTL_ANTHROPIC_API_KEY"))
                    .map_err(|_| "未设置 ANTHROPIC_API_KEY 环境变量")?
            }
            LlmProvider::Local => "".to_string(),
        };
        LlmClient::new(provider, api_key)
    } else {
        LlmClient::from_env().map_err(|e| format!("LLM 配置错误: {}", e))?
    };

    // 调用大模型获取诊断
    let mut diagnosis = llm_client.diagnose(pid, causes.clone(), processes).await?;
    diagnosis.pid = pid;
    diagnosis.causes = causes;

    Ok(diagnosis)
}
