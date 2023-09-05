/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0 
 *
 * You should have received a copy of the Elastic License 2.0 along with 
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

// Rhai modules in the `rhai-fs` package.
mod pkg {
    include!("src/api.rs");
}

fn main() {
    if let Ok(docs_path) = std::env::var("DOCS_DIR") {
        let mut engine = rhai::Engine::new();

        engine.register_static_module("sqlite", rhai::exported_module!(pkg::sqlite_api).into());

        let docs = rhai_autodocs::options()
            .format_sections_with(rhai_autodocs::SectionFormat::Tabs)
            .include_standard_packages(false)
            .order_functions_with(rhai_autodocs::FunctionOrder::ByIndex)
            .for_markdown_processor(rhai_autodocs::options::MarkdownProcessor::Docusaurus)
            .generate(&engine)
            .expect("failed to generate documentation");

        write_docs(&docs_path, &docs);
    }
}

fn write_docs(path: &str, docs: &rhai_autodocs::ModuleDocumentation) {
    std::fs::write(
        std::path::PathBuf::from_iter([path, &format!("fn::{}.mdx", &docs.name)]),
        &docs.documentation,
    )
    .expect("failed to write documentation");

    for doc in &docs.sub_modules {
        write_docs(path, doc);
    }
}
