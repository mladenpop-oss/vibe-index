use crate::VibeIndex;
use crate::MatchResult;
use std::time::Instant;

/// llama.cpp integration module
/// Connects Vibe Index to llama.cpp's prompt template system
pub struct LlamaCppIntegration {
    index: VibeIndex,
    /// llama.cpp server endpoint
    server_url: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LlamaCppPromptTemplate {
    pub template: String,
    pub context_tokens: Vec<String>,
    pub query: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LlamaCppCompletionRequest {
    pub prompt: String,
    pub n_predict: i32,
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub seed: i32,
}

#[derive(serde::Deserialize, Debug)]
pub struct LlamaCppCompletionResponse {
    pub content: String,
    pub stop: bool,
    pub tokens_predicted: i32,
}

impl LlamaCppIntegration {
    pub fn new(server_url: String) -> Self {
        Self {
            index: VibeIndex::new(),
            server_url,
        }
    }

    pub fn add_token(&mut self, token: &str) {
        self.index.add_token(token);
    }

    /// Build a llama.cpp-compatible prompt with Vibe Index optimized context
    pub fn build_vibe_prompt(
        &mut self,
        user_query: &str,
        full_context: &[String],
        search_queries: &[Vec<String>],
    ) -> (String, Vec<MatchResult>) {
        let start = Instant::now();

        // Search for relevant positions
        let mut all_matches: Vec<MatchResult> = Vec::new();
        for query in search_queries {
            let results = self.index.phrase_search(query);
            all_matches.extend(results);
        }
        all_matches.retain(|m| m.confidence >= 0.5);
        all_matches.sort_by_key(|m| m.position);

        // Build context section with position markers
        let mut context_section = String::new();
        context_section.push_str("<context>\n");

        for m in &all_matches {
            let pos = m.position;
            let context_window = 10;
            let start = pos.saturating_sub(context_window);
            let end = (pos + context_window).min(full_context.len());

            if start < end {
                let window: Vec<String> = full_context[start..end].to_vec();
                context_section.push_str(&format!(
                    "  [POS {}] {}\n",
                    pos,
                    window.join(" ")
                ));
            }
        }

        context_section.push_str("</context>\n");

        // Build the full prompt using llama.cpp template style
        let prompt = format!(
            "You are a code assistant. Use the provided context to answer the query.\n\n{}\n\nQuery: {}\nAnswer:",
            context_section, user_query
        );

        let latency = start.elapsed().as_secs_f64() * 1000.0;
        println!("[LLAMA] Built prompt: {} chars, {} matches, {:.2}ms",
            prompt.len(), all_matches.len(), latency);

        (prompt, all_matches)
    }

    /// Send completion request to llama.cpp server
    pub async fn complete(&self, prompt: &str) -> Result<LlamaCppCompletionResponse, anyhow::Error> {
        let request = LlamaCppCompletionRequest {
            prompt: prompt.to_string(),
            n_predict: 512,
            temperature: 0.7,
            top_k: 40,
            top_p: 0.95,
            repeat_penalty: 1.1,
            seed: 42,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/completion", self.server_url))
            .json(&request)
            .send()
            .await?
            .json::<LlamaCppCompletionResponse>()
            .await?;

        Ok(response)
    }

    /// Full pipeline: index context -> build prompt -> get completion
    pub async fn ask(
        &mut self,
        context: &[String],
        user_query: &str,
        search_queries: &[Vec<String>],
    ) -> Result<(String, Vec<MatchResult>), anyhow::Error> {
        // Index the context
        for token in context {
            self.add_token(token);
        }

        // Build optimized prompt
        let (prompt, matches) = self.build_vibe_prompt(user_query, context, search_queries);

        // Get completion from llama.cpp
        let response = self.complete(&prompt).await?;

        Ok((response.content, matches))
    }

    /// Build prompt template for llama.cpp's built-in template system
    /// This can be saved as a .tmpl file for llama.cpp
    pub fn build_template_file(
        &self,
        search_queries: &[Vec<String>],
    ) -> String {
        let mut template = String::from("{{- if .System }}{{ .System }}\n{{ end }}");
        template.push_str("\n{{- if .Prompt }}");
        template.push_str("\n<context>");

        // Each search query becomes a conditional context injection
        for query in search_queries {
            let query_str = query.join(" ");
            template.push_str(&format!(
                "\n  {{- if .Context.{} }}\n  [MATCH: {}] {{ .Context.{} }}\n  {{ end }}",
                query_str, query_str, query_str
            ));
        }

        template.push_str("\n</context>");
        template.push_str("\n{{ .Prompt }}");
        template.push_str("\n{{ end }}");

        template
    }
}

/// Example: Pre-built templates for common coding tasks
pub mod templates {
    pub const REFACTORING_PROMPT: &str = r#"
You are a code refactoring assistant. The context below contains relevant code sections.
Refactor the code according to the query while preserving functionality.

Context:
{{.Context}}

Query: {{.Prompt}}
"#;

    pub const BUGFIND_PROMPT: &str = r#"
You are a code debugging assistant. Analyze the context for bugs and issues.

Context:
{{.Context}}

Query: {{.Prompt}}
"#;

    pub const DOCS_PROMPT: &str = r#"
You are a documentation assistant. Generate documentation based on the provided code context.

Context:
{{.Context}}

Query: {{.Prompt}}
"#;
}
