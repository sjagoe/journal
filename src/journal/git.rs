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

    let head = repo.head()?;
    let branch = head.shorthand().unwrap_or("main");

    repo.reference(&format!("refs/heads/{}", branch), commit_oid, true, message)?;

    Ok(())
}
