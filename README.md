# Manage age-encrypted org-mode journal entries

A TUI application to manage encrypted journal entries.

This assumes org files and uses emacs org-mode to export to HTML.  New
files will be created with a trivial org outline, and referencing the
`style.css.org` found in the `./example/` directory.


## First setup

1. Create an age key

```
mkdir ~/.age

nix shell nixpkgs#age

# -pq -  Generate a post-quantum hybrid ML-KEM-768 + X25519 key pair.
age-keygen -pq -o ~/.age/age-personal
```

2. Create a new git repository with the content from the `./example/`
   directory.

3. Copy the public key printed by `age-keygen` above into the
   `publicKeys` list in your journal repository `rules.nix`.

4. Configure this tool with a JSON file in
   `~/.config/personal-journal/config.json`.

```json
{
  "identity": "/home/USERNAME/.age/age-personal",
  "journal_path": "/home/USERNAME/journal-entries",
  "editor": "emacsclient --create-frame",
  "export": "firefox-release --new-window"
}
```

`identity` and `journal_path` are required.  `export` will fall back
to `xdg-open` to launch the default `html` file handler, and `editor`
will fall back to the content of the `VISUAL` and then the `EDITOR`
environment variables.

Then launch `journal` with nix:

```
nix run
# or nix run github:sjagoe/journal
```
