use crate::domain::job::JobKind;

/// Builds the backend-authoritative idempotency namespace.
///
/// Caller-provided keys are never trusted as globally unique. Project and job
/// kind are always part of the persisted key so the same caller key can be
/// reused safely by another project or operation kind.
pub fn scoped_idempotency_key(project_id: &str, kind: &JobKind, caller_key: &str) -> String {
    format!("v2:project={project_id}:kind={kind:?}:key={caller_key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_caller_key_is_isolated_by_project() {
        let a = scoped_idempotency_key("project-a", &JobKind::Scoring, "shared");
        let b = scoped_idempotency_key("project-b", &JobKind::Scoring, "shared");
        assert_ne!(a, b);
    }

    #[test]
    fn same_caller_key_is_isolated_by_job_kind() {
        let scoring = scoped_idempotency_key("project-a", &JobKind::Scoring, "shared");
        let ocr = scoped_idempotency_key("project-a", &JobKind::StudentAnswerOcr, "shared");
        assert_ne!(scoring, ocr);
    }

    #[test]
    fn same_scope_remains_stable() {
        let a = scoped_idempotency_key("project-a", &JobKind::Scoring, "shared");
        let b = scoped_idempotency_key("project-a", &JobKind::Scoring, "shared");
        assert_eq!(a, b);
    }
}
