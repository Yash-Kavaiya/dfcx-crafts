//! Helpers for Google-style resource names such as
//! `projects/p/locations/global/agents/a/flows/f/pages/x`.

/// Well-known ids used by Conversational Agents.
pub const DEFAULT_START_FLOW_ID: &str = "00000000-0000-0000-0000-000000000000";
pub const DEFAULT_WELCOME_INTENT_ID: &str = "00000000-0000-0000-0000-000000000000";
pub const DEFAULT_NEGATIVE_INTENT_ID: &str = "00000000-0000-0000-0000-000000000001";
pub const START_PAGE: &str = "START_PAGE";
pub const END_SESSION: &str = "END_SESSION";
pub const END_FLOW: &str = "END_FLOW";
pub const PREVIOUS_PAGE: &str = "PREVIOUS_PAGE";
pub const CURRENT_PAGE: &str = "CURRENT_PAGE";
pub const END_FLOW_WITH_HUMAN_ESCALATION: &str = "END_FLOW_WITH_HUMAN_ESCALATION";
pub const END_FLOW_WITH_FAILURE: &str = "END_FLOW_WITH_FAILURE";
pub const END_FLOW_WITH_CANCELLATION: &str = "END_FLOW_WITH_CANCELLATION";

/// Special page ids that are not stored resources.
pub const SPECIAL_PAGES: &[&str] = &[
    START_PAGE,
    END_SESSION,
    END_FLOW,
    PREVIOUS_PAGE,
    CURRENT_PAGE,
    END_FLOW_WITH_HUMAN_ESCALATION,
    END_FLOW_WITH_FAILURE,
    END_FLOW_WITH_CANCELLATION,
];

/// Last path segment of a resource name.
pub fn id_of(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Parent of a resource name (`a/b/c/d` -> `a/b`).
pub fn parent_of(name: &str) -> &str {
    let mut idx = name.len();
    for _ in 0..2 {
        match name[..idx].rfind('/') {
            Some(i) => idx = i,
            None => return "",
        }
    }
    &name[..idx]
}

/// Collection segment of a resource name (`a/b/c/d` -> `c`).
pub fn collection_of(name: &str) -> &str {
    let segs: Vec<&str> = name.split('/').collect();
    if segs.len() >= 2 {
        segs[segs.len() - 2]
    } else {
        ""
    }
}

/// Returns the ancestor whose collection is `collection`, e.g.
/// `ancestor(page_name, "agents")` gives the agent name.
pub fn ancestor(name: &str, collection: &str) -> Option<String> {
    let segs: Vec<&str> = name.split('/').collect();
    let mut i = 0;
    while i + 1 < segs.len() {
        if segs[i] == collection {
            return Some(segs[..i + 2].join("/"));
        }
        i += 2;
    }
    None
}

pub fn is_special_page(id: &str) -> bool {
    SPECIAL_PAGES.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        let n = "projects/p/locations/global/agents/a/flows/f/pages/x";
        assert_eq!(id_of(n), "x");
        assert_eq!(parent_of(n), "projects/p/locations/global/agents/a/flows/f");
        assert_eq!(collection_of(n), "pages");
        assert_eq!(ancestor(n, "agents").unwrap(), "projects/p/locations/global/agents/a");
        assert_eq!(parent_of("projects/p"), "");
    }
}
