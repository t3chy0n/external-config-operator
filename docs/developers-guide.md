# Developer's Guide: Creating Custom Store Drivers (Extractors) for Configuration Stores
This guide provides a comprehensive overview of how to develop your own store drivers (called extractors) to integrate with a configuration store system. The extractors are essentially servers that serve configuration data in formats like JSON, YAML, TOML, properties, or environment files. Examples are included in JavaScript (Node.js), Go, and Java using Spring Boot.

## Key Responsibilities of an Extractor
   An extractor is responsible for:

- Exposing an HTTP API endpoint that serves configuration data.
- Supporting multiple formats (JSON, YAML, TOML, properties, env files).
- Accepting input parameters (e.g., request parameters or headers).
- Validating input and providing meaningful error responses.


## Basic API Specification
   A typical extractor should implement the following HTTP API endpoint:

### GET /<your-path>/../<as_needed>
 Fetches configuration data based on provided parameters.

#### Query Parameters:
- format (required): Specifies the format of the response (e.g., json, yaml, toml, properties, env).
- params (optional): Key-value pairs for additional configuration filtering.
#### Response Codes:
- 200 OK: Successfully fetched configuration.
- 400 Bad Request: Invalid request (e.g., missing required parameters).
- 500 Internal Server Error: Failure in fetching or processing configuration.