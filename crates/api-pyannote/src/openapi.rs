use utoipa::openapi::{
    OpenApi,
    path::{Operation, PathItem},
};

pub fn openapi() -> OpenApi {
    let mut doc = hypr_pyannote_cloud::openapi();

    doc.servers = None;
    doc.security = None;

    if let Some(components) = doc.components.as_mut() {
        components.security_schemes.clear();
    }

    for item in doc.paths.paths.values_mut() {
        with_each_operation(item, |operation| {
            operation.security = None;
            operation.tags = Some(vec!["pyannote".to_string()]);
        });
    }

    doc
}

fn with_each_operation(item: &mut PathItem, mut f: impl FnMut(&mut Operation)) {
    if let Some(op) = item.get.as_mut() {
        f(op);
    }
    if let Some(op) = item.put.as_mut() {
        f(op);
    }
    if let Some(op) = item.post.as_mut() {
        f(op);
    }
    if let Some(op) = item.delete.as_mut() {
        f(op);
    }
    if let Some(op) = item.options.as_mut() {
        f(op);
    }
    if let Some(op) = item.head.as_mut() {
        f(op);
    }
    if let Some(op) = item.patch.as_mut() {
        f(op);
    }
    if let Some(op) = item.trace.as_mut() {
        f(op);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_upstream_pyannote_doc() {
        let doc = super::openapi();
        let diarize = doc.paths.paths.get("/v1/diarize").unwrap();
        let post = diarize.post.as_ref().unwrap();

        assert!(doc.servers.is_none());
        assert!(post.security.is_none());
        assert_eq!(post.tags.as_ref().unwrap(), &vec!["pyannote".to_string()]);
    }
}
