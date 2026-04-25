use vibe_index::llama_cpp::LlamaCppIntegration;

#[tokio::test]
async fn test_llama_cpp_integration() {
    let server_url = "http://127.0.0.1:8080";
    let mut integration = LlamaCppIntegration::new(server_url.to_string());

    let context: Vec<String> = vec![
        "fn".to_string(),
        "main".to_string(),
        "(".to_string(),
        ")".to_string(),
        "->".to_string(),
        "i32".to_string(),
        "}".to_string(),
        "fn".to_string(),
        "add".to_string(),
        "(a".to_string(),
        "b".to_string(),
        ")".to_string(),
        "->".to_string(),
        "i32".to_string(),
        "{".to_string(),
        "a+b".to_string(),
        "}".to_string(),
        "fn".to_string(),
        "greet".to_string(),
        "(name".to_string(),
        ":&str".to_string(),
        ")".to_string(),
        "{".to_string(),
        "format!(\"Hello, {}\", name)".to_string(),
        "}".to_string(),
    ];

    for token in &context {
        integration.add_token(token);
    }

    let search_queries = vec![
        vec!["fn".into(), "main".into()],
        vec!["fn".into(), "add".into()],
    ];

    let (prompt, matches) = integration.build_vibe_prompt(
        "What does the add function do?",
        &context,
        &search_queries,
    );

    println!("Matches found: {}", matches.len());
    println!("Prompt size: {} bytes", prompt.len());
    assert!(matches.len() >= 1, "Should find at least one match");
    assert!(prompt.contains("<context>"), "Prompt should contain context section");

    let response = integration.complete(&prompt).await;
    match response {
        Ok(resp) => {
            println!("Response: {}", resp.content);
            println!("Tokens predicted: {}", resp.tokens_predicted);
        }
        Err(e) => {
            println!("Completion error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_llama_cpp_full_pipeline() {
    let server_url = "http://127.0.0.1:8080";
    let mut integration = LlamaCppIntegration::new(server_url.to_string());

    let context: Vec<String> = vec![
        "use".to_string(),
        "std".to_string(),
        "::".to_string(),
        "collections".to_string(),
        "::".to_string(),
        "HashMap".to_string(),
        ".".to_string(),
        "fn".to_string(),
        "main".to_string(),
        "(".to_string(),
        ")".to_string(),
        "{".to_string(),
        "let".to_string(),
        "mut".to_string(),
        "cache".to_string(),
        ":=".to_string(),
        "HashMap".to_string(),
        "::".to_string(),
        "new".to_string(),
        "()".to_string(),
        ";".to_string(),
        "cache".to_string(),
        ".insert".to_string(),
        "(".to_string(),
        "\"key1\"".to_string(),
        ", 42".to_string(),
        ")".to_string(),
        ";".to_string(),
        "println!(\"Cache size: {}\", cache.len())".to_string(),
        "}".to_string(),
        "pub".to_string(),
        "fn".to_string(),
        "process_data".to_string(),
        "(data: &Vec<String>)".to_string(),
        "->".to_string(),
        "Result<(), Error>".to_string(),
        "{".to_string(),
        "let".to_string(),
        "mut".to_string(),
        "results".to_string(),
        ":=".to_string(),
        "Vec::new()".to_string(),
        ";".to_string(),
        "for".to_string(),
        "item".to_string(),
        "in".to_string(),
        "data".to_string(),
        "{".to_string(),
        "results.push(item.to_uppercase())".to_string(),
        ";".to_string(),
        "}".to_string(),
        "Ok(())".to_string(),
        "}".to_string(),
        "fn".to_string(),
        "calculate_average".to_string(),
        "(numbers: &[i32])".to_string(),
        "->".to_string(),
        "f64".to_string(),
        "{".to_string(),
        "if".to_string(),
        "numbers.is_empty()".to_string(),
        "{".to_string(),
        "return".to_string(),
        "0.0".to_string(),
        ";".to_string(),
        "}".to_string(),
        "let".to_string(),
        "sum: i32".to_string(),
        "= numbers.iter().sum()".to_string(),
        ";".to_string(),
        "sum as f64 / numbers.len() as f64".to_string(),
        "}".to_string(),
        "struct".to_string(),
        "DatabaseConnection".to_string(),
        "{".to_string(),
        "pool".to_string(),
        ":".to_string(),
        "R2D2".to_string(),
        "}".to_string(),
        "impl".to_string(),
        "DatabaseConnection".to_string(),
        "{".to_string(),
        "fn".to_string(),
        "query".to_string(),
        "(&self, sql: &str)".to_string(),
        "->".to_string(),
        "QueryResult".to_string(),
        "{".to_string(),
        "let".to_string(),
        "conn".to_string(),
        "= self.pool.get()".to_string(),
        ";".to_string(),
        "conn.unwrap().query(sql)".to_string(),
        "}".to_string(),
        "}".to_string(),
        "async".to_string(),
        "fn".to_string(),
        "fetch_users".to_string(),
        "(db: &DatabaseConnection)".to_string(),
        "->".to_string(),
        "Vec<User>".to_string(),
        "{".to_string(),
        "db.query(\"SELECT * FROM users\")".to_string(),
        "}".to_string(),
        "fn".to_string(),
        "create_user".to_string(),
        "(name: &str, email: &str)".to_string(),
        "->".to_string(),
        "Result<User, Error>".to_string(),
        "{".to_string(),
        "validate_email(email)?".to_string(),
        "Ok(User { name: name.to_string(), email: email.to_string() })".to_string(),
        "}".to_string(),
        "fn".to_string(),
        "validate_email".to_string(),
        "(email: &str)".to_string(),
        "->".to_string(),
        "Result<(), Error>".to_string(),
        "{".to_string(),
        "email.contains('@')".to_string(),
        "}".to_string(),
        "struct".to_string(),
        "User".to_string(),
        "{".to_string(),
        "pub".to_string(),
        "name: String".to_string(),
        ",".to_string(),
        "pub".to_string(),
        "email: String".to_string(),
        "}".to_string(),
        "trait".to_string(),
        "Repository<T>".to_string(),
        "{".to_string(),
        "fn".to_string(),
        "find_by_id(&self, id: i32) -> Option<T>".to_string(),
        ";".to_string(),
        "fn".to_string(),
        "save(&self, entity: &T) -> Result<(), Error>".to_string(),
        ";".to_string(),
        "fn".to_string(),
        "delete(&self, id: i32) -> Result<(), Error>".to_string(),
        ";".to_string(),
        "}".to_string(),
    ];

    for token in &context {
        integration.add_token(token);
    }

    let search_queries = vec![
        vec!["fn".into(), "calculate_average".into()],
        vec!["fn".into(), "create_user".into()],
        vec!["struct".into(), "User".into()],
    ];

    let (prompt, matches) = integration.build_vibe_prompt(
        "What functions are available for user management and what does calculate_average do?",
        &context,
        &search_queries,
    );

    println!("=== FULL PIPELINE TEST ===");
    println!("Context tokens indexed: {}", context.len());
    println!("Search queries: {}", search_queries.len());
    println!("Matches found: {}", matches.len());
    println!("Prompt size: {} bytes", prompt.len());
    println!("\n=== PROMPT ===");
    println!("{}", prompt);
    println!("=== END PROMPT ===\n");

    assert!(matches.len() >= 2, "Should find at least 2 matches");
    assert!(prompt.contains("<context>"), "Prompt should contain context section");
    assert!(prompt.contains("calculate_average"), "Prompt should mention calculate_average");

    let response = integration.complete(&prompt).await;
    match response {
        Ok(resp) => {
            println!("=== MODEL RESPONSE ===");
            println!("{}", resp.content);
            println!("Tokens predicted: {}", resp.tokens_predicted);
            println!("=== END RESPONSE ===");
            assert!(!resp.content.is_empty(), "Response should not be empty");
        }
        Err(e) => {
            println!("Completion error: {}", e);
        }
    }
}
