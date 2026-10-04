use crate::{
    git_ops,
    models::{AppData, GhProfileStatus, Profile, RepositoryState, RepositoryStatus},
    operations,
    storage::StateStore,
};
use std::collections::HashMap;

pub fn classify(assigned: bool, identity_in_sync: bool, github_connected: bool) -> RepositoryState {
    if !assigned {
        RepositoryState::Unassigned
    } else if !identity_in_sync {
        RepositoryState::Reapply
    } else if !github_connected {
        RepositoryState::Attention
    } else {
        RepositoryState::Ready
    }
}

fn github_matches(profile: &Profile, github: &GhProfileStatus) -> bool {
    github.authenticated
        && profile.github_username.as_deref().is_some_and(|expected| {
            github
                .username
                .as_deref()
                .is_some_and(|actual| expected.eq_ignore_ascii_case(actual))
        })
}

pub fn inspect_repository_statuses(store: &StateStore, data: &AppData) -> Vec<RepositoryStatus> {
    // GitHub inspection runs once for each assigned Profile, not once per repository.
    let mut github_by_profile: HashMap<String, Option<GhProfileStatus>> = HashMap::new();
    for repository in &data.repositories {
        let Some(profile) = repository
            .profile_id
            .as_ref()
            .and_then(|id| data.profiles.iter().find(|profile| &profile.id == id))
        else {
            continue;
        };
        github_by_profile
            .entry(profile.id.clone())
            .or_insert_with(|| {
                Some(
                    operations::inspect_github_profile(
                        store,
                        profile.id.clone(),
                        profile.gh_config_dir.clone(),
                    )
                    .unwrap_or_else(|detail| GhProfileStatus {
                        available: false,
                        authenticated: false,
                        username: None,
                        detail: Some(detail),
                        config_dir: profile.gh_config_dir.clone(),
                    }),
                )
            });
    }

    data.repositories
        .iter()
        .map(|repository| {
            let profile = repository
                .profile_id
                .as_ref()
                .and_then(|id| data.profiles.iter().find(|profile| &profile.id == id));
            let github = profile
                .and_then(|profile| github_by_profile.get(&profile.id))
                .cloned()
                .flatten();
            match git_ops::inspect_identity_status(repository, profile) {
                Ok((branch, changes, mismatches)) => {
                    let identity_in_sync = profile.is_some() && mismatches.is_empty();
                    let state = classify(
                        profile.is_some(),
                        identity_in_sync,
                        profile
                            .zip(github.as_ref())
                            .is_some_and(|(profile, github)| github_matches(profile, github)),
                    );
                    RepositoryStatus {
                        repository_id: repository.id.clone(),
                        state,
                        branch: Some(branch),
                        uncommitted_changes: Some(changes.len()),
                        identity_in_sync,
                        mismatched_keys: mismatches.into_iter().map(|item| item.key).collect(),
                        github,
                        error: None,
                    }
                }
                Err(error) => RepositoryStatus {
                    repository_id: repository.id.clone(),
                    state: if profile.is_some() {
                        RepositoryState::Attention
                    } else {
                        RepositoryState::Unassigned
                    },
                    branch: repository.branch.clone(),
                    uncommitted_changes: None,
                    identity_in_sync: false,
                    mismatched_keys: Vec::new(),
                    github,
                    error: Some(error),
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_repository_states() {
        assert_eq!(classify(true, true, true), RepositoryState::Ready);
        assert_eq!(classify(true, false, true), RepositoryState::Reapply);
        assert_eq!(classify(false, false, false), RepositoryState::Unassigned);
        assert_eq!(classify(true, true, false), RepositoryState::Attention);
        assert_eq!(classify(true, false, false), RepositoryState::Reapply);
    }

    #[test]
    fn github_connection_requires_the_assigned_account() {
        let profile = Profile {
            id: "example".into(),
            label: "Example".into(),
            accent: "#D8A33F".into(),
            git_name: "Example Contributor".into(),
            git_email: "author@example.com".into(),
            github_username: Some("example-user".into()),
            ssh_key_path: None,
            gh_config_dir: None,
            auto_approve: Default::default(),
        };
        let github = GhProfileStatus {
            available: true,
            authenticated: true,
            username: Some("EXAMPLE-USER".into()),
            detail: None,
            config_dir: None,
        };
        assert!(github_matches(&profile, &github));
        assert!(!github_matches(
            &profile,
            &GhProfileStatus {
                username: Some("another-user".into()),
                ..github.clone()
            }
        ));
        assert!(!github_matches(
            &profile,
            &GhProfileStatus {
                authenticated: false,
                ..github
            }
        ));
    }
}
