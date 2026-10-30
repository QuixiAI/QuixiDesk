use super::*;
impl McpHost {
    pub fn browse_resources(&self, server: &str, cwd: &Path) -> Result<Value> {
        let name = self.canonical_name(server)?;
        if self.is_builtin_server(&name) {
            return Ok(json!({"resources":[],"resourceTemplates":[],"prompts":[],"roots":[]}));
        }
        let entry = self.external_entry(&name, cwd)?;
        client::block_on(client::Manager::discover(&entry, &AtomicBool::new(false)))?;
        let status = entry.status();
        Ok(
            json!({"server":name,"status":status["status"],"resources":status["resources"],"resourceTemplates":status["resourceTemplates"],"prompts":status["prompts"],"roots":status["roots"],"capabilities":status["capabilities"]}),
        )
    }
    pub fn expand_resource_template(
        &self,
        server: &str,
        template: &str,
        args: &Value,
        cwd: &Path,
    ) -> Result<Value> {
        let catalog = self.browse_resources(server, cwd)?;
        if !catalog["resourceTemplates"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["uriTemplate"] == template)
        {
            bail!("Unknown MCP resource template");
        }
        validate_template(template)?;
        let argument_size = serde_json::to_vec(args)?.len();
        if argument_size > 65536
            || argument_size.saturating_mul(template.matches(['{', ',']).count().max(1)) > 1048576
        {
            bail!("Resource template expansion is too large");
        }
        let mut expansion = uritemplate::UriTemplate::new(template);
        for (k, v) in args
            .as_object()
            .context("Template arguments must be an object")?
        {
            match v {
                Value::String(s) => {
                    expansion.set(k, s.clone());
                }
                Value::Array(values) => {
                    let list: Vec<String> = values
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .context("Template list values must be strings")
                        })
                        .collect::<Result<_>>()?;
                    expansion.set(k, list);
                }
                _ => bail!("Template arguments must be strings or string lists"),
            }
        }
        let uri = expansion.build();
        if uri.len() > 65536 {
            bail!("Expanded URI exceeds 64 KiB");
        }
        url::Url::parse(&uri).context("Template did not produce an absolute URI")?;
        Ok(json!({"uri":uri}))
    }
    pub fn subscribe_resource(
        &self,
        server: &str,
        uri: &str,
        cwd: &Path,
        enabled: bool,
    ) -> Result<()> {
        let name = self.canonical_name(server)?;
        let entry = self.external_entry(&name, cwd)?;
        client::block_on(client::Manager::subscribe(&entry, uri, enabled))
    }
    pub fn resource_updates(&self) -> Vec<Value> {
        self.manager.resource_updates()
    }
    pub fn set_roots(&self, server: &str, cwd: &Path, roots: &[PathBuf]) -> Result<()> {
        if roots.len() > 32 {
            bail!("At most 32 workspace roots are supported");
        }
        let mut values = Vec::new();
        for path in roots {
            let path = path
                .canonicalize()
                .context("Workspace root does not exist")?;
            if !path.is_dir() {
                bail!("Workspace root must be a directory");
            }
            let uri = url::Url::from_directory_path(&path)
                .map_err(|_| anyhow::anyhow!("Invalid workspace root"))?;
            values.push(json!({"uri":uri.as_str(),"name":path.file_name().unwrap_or_default().to_string_lossy(),"path":path}));
        }
        let name = self.canonical_name(server)?;
        let entry = self.external_entry(&name, cwd)?;
        client::block_on(client::Manager::set_roots(&entry, values))
    }
}

// RFC 6570 variable syntax is ASCII (non-ASCII names must be percent encoded).
// Validate before passing untrusted templates to the expansion library.
fn validate_template(template: &str) -> Result<()> {
    if template.len() > 8192 {
        bail!("Resource template exceeds 8 KiB");
    }
    let mut expression = None;
    for (i, ch) in template.char_indices() {
        match ch {
            '{' if expression.is_none() => expression = Some(i + 1),
            '{' => bail!("Nested resource template expression"),
            '}' => {
                let start = expression
                    .take()
                    .context("Unexpected template closing brace")?;
                let expr = &template[start..i];
                let expr = expr
                    .strip_prefix(['+', '#', '.', '/', ';', '?', '&'])
                    .unwrap_or(expr);
                for spec in expr.split(',') {
                    let name = if let Some(name) = spec.strip_suffix('*') {
                        name
                    } else if let Some((name, prefix)) = spec.split_once(':') {
                        if prefix.is_empty()
                            || prefix.len() > 4
                            || !prefix.bytes().all(|b| b.is_ascii_digit())
                            || prefix.starts_with('0')
                        {
                            bail!("Invalid template prefix");
                        }
                        name
                    } else {
                        spec
                    };
                    if name.is_empty() || !name.is_ascii() {
                        bail!("Invalid resource template variable");
                    }
                    let bytes = name.as_bytes();
                    let mut pos = 0;
                    while pos < bytes.len() {
                        match bytes[pos] {
                            b'%' if pos + 2 < bytes.len()
                                && bytes[pos + 1].is_ascii_hexdigit()
                                && bytes[pos + 2].is_ascii_hexdigit() =>
                            {
                                pos += 3
                            }
                            b'.' | b'_' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => pos += 1,
                            _ => bail!("Invalid resource template variable"),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if expression.is_some() {
        bail!("Unclosed resource template expression");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_validation_rejects_malformed_and_non_ascii_expressions() {
        for bad in [
            "x://{é}",
            "x://{a",
            "x://a}",
            "x://{{a}}",
            "x://{}",
            "x://{a:0}",
            "x://{a:foo}",
            "x://{%zz}",
        ] {
            assert!(validate_template(bad).is_err(), "{bad}");
        }
        for good in ["x://{id}{?view,tags*}", "x://{+path}", "x://{%C3%A9:12}"] {
            validate_template(good).unwrap();
        }
    }
}
