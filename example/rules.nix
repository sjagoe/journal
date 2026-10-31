let
  publicKeys = [
    # Add your age public keys here
    "age1....."
  ];

  directories = ["documents" "general" "narrative"];

  flatten = x:
    if builtins.isList x
    then builtins.concatMap (y: flatten y) x
    else [x];

  filterEntries = filter: entries:
    builtins.filter (filename: filename != null)
      (builtins.attrValues
        (builtins.mapAttrs filter entries));

  filterEntryFiles = filename: type:
    if (((builtins.match "^[^\\.].*\\.age$" filename) != null) &&
        ((builtins.match "^regular$") type) != null)
    then filename
    else null;

  filterEntryDirectories = dirname: type:
    if ((builtins.elem dirname directories) &&
        ((builtins.match "^directory$") type) != null)
    then dirname
    else null;

  availableDirectories =
    filterEntries
      filterEntryDirectories
      (builtins.readDir ./.);

  entriesPaths = flatten (map
    (dir:
      (map (item: "${dir}/${item}")
        (filterEntries
          filterEntryFiles
          (builtins.readDir (./. + "/${dir}")))))
    availableDirectories);

  entries = builtins.map
    (item: { name = item; value = { inherit publicKeys; }; })
    entriesPaths;
in
builtins.listToAttrs entries
