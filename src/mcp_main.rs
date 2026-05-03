use modelcontextprotocol_server::{server::ServerBuilder, transport::StdioTransport};

mod mcp_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let vibe_server = mcp_server::VibeMcpServer::new();

    let mut builder =
        ServerBuilder::new("vibe-index", "0.1.0").with_transport(StdioTransport::new());

    for tool in vibe_server.tools() {
        builder = builder.with_tool(
            &tool.name,
            tool.description.as_deref(),
            tool.input_schema,
            tool.handler,
        );
    }

    let mcp_server = builder.build()?;

    println!("Vibe Index MCP Server started");
    println!("Tools available:");
    for tool in vibe_server.tools() {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.as_deref().unwrap_or("")
        );
    }
    println!();

    mcp_server.run().await?;
    Ok(())
}
