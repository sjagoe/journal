use std::error;

use git2::{ErrorCode, Repository};
use gpgme::{Context, Protocol};

fn sign_commit(commit_content: &[u8], signature: &mut Vec<u8>) -> Result<(), gpgme::Error> {
    let mut ctx = Context::from_protocol(Protocol::OpenPgp)?;
    ctx.set_armor(true);
    ctx.sign_detached(commit_content, signature)?;

    Ok(())
}

pub fn commit(repo: &Repository, message: &str) -> Result<(), Box<dyn error::Error>> {
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent_commit = match repo.revparse_single("HEAD") {
        Ok(obj) => Some(obj.into_commit().unwrap()),
        // First commit so no parent commit
        Err(e) if e.code() == ErrorCode::NotFound => None,
        Err(e) => return Err(Box::new(e)),
    };

    let mut parents = Vec::new();
    if let Some(ref parent) = parent_commit {
        parents.push(parent);
    }

    let sig = repo.signature()?;

    let config = repo.config()?;

    let default_branch = match config.get_string("init.defaultBranch") {
        Ok(name) => name,
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                "master".to_string()
            } else {
                return Err(Box::new(e));
            }
        }
    };

    let commit_oid = if config.get_bool("commit.gpgsign")? {
        let commit_content = repo.commit_create_buffer(&sig, &sig, message, &tree, &parents)?;
        let mut signature = Vec::new();

        sign_commit(&commit_content, &mut signature)?;

        let contents = commit_content.as_str()?;
        let out = str::from_utf8(&signature)?;

        repo.commit_signed(contents, out, Some("gpgsig"))?
    } else {
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents[..])?
    };

    let branch = match repo.head() {
        Ok(head) => {
            let value = head.shorthand().unwrap_or(&default_branch);
            value.to_owned()
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::UnbornBranch {
                default_branch
            } else {
                return Err(Box::new(e));
            }
        }
    };

    repo.reference(&format!("refs/heads/{}", branch), commit_oid, true, message)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn test_commit() {
        let dir = tempfile::tempdir().expect("Unable to create temp dir");

        let repo = Repository::init(&dir).expect("Unable to init repository");

        let mut repo_config = repo.config().expect("Unable to get git repo config");
        repo_config.set_bool("commit.gpgsign", false).unwrap();
        repo_config.set_str("user.name", "Test User").unwrap();
        repo_config
            .set_str("user.email", "user@example.test")
            .unwrap();

        let mut index = repo.index().expect("Couldn't get git index");

        let new_file_name = Path::new("new-file");
        let mut new_file_path = PathBuf::new();
        new_file_path.push(&dir);
        new_file_path.push(&new_file_name);

        std::fs::write(&new_file_path, b"content").expect("Failed to create file");

        index
            .add_path(&new_file_name)
            .expect("Failed to add to index");

        let expected_message = "commit message";
        commit(&repo, expected_message).expect("Failed to commit");

        let head = repo.head().unwrap();

        assert_eq!(head.is_branch(), true);

        let ref_name = head.name().unwrap();

        assert_eq!(ref_name, "refs/heads/master");

        let commit = head.peel_to_commit().unwrap();

        let commit_message = commit.message().unwrap();

        assert_eq!(commit_message, expected_message);
    }
}
