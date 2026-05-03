## Features

This crate provides the following features:

- `camel_case`: Serializes the input schema field as "inputSchema" (for JavaScript/TypeScript clients)
- `snake_case`: Serializes the input schema field as "input_schema" (for Ruby/Python clients)

Example usage:

```toml
# For camel case serialization (JavaScript clients)
mcp-protocol = { version = "0.1.0", features = ["camel_case"] }

# For snake case serialization (Python clients)
mcp-protocol = { version = "0.1.0", features = ["snake_case"] }
