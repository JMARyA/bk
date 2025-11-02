{
  # Make backup configuration
  makeBk =
    {
      paths,
      repo,
      extraOptions ? { },
      extraPathOptions ? { },
      extraTargetOptions ? { },
    }:
    let
      pathDefs = map (path: {
        ${path} = {
          inherit path;
        }
        // extraPathOptions;
      }) paths;

      # Merge all pathDefs into a single set
      mergedPaths = builtins.foldl' (a: b: a // b) { } pathDefs;
    in
    {
      # Top-level restic target
      restic_target.${repo} = {
        repo = repo;
      }
      // extraTargetOptions;

      # restic configuration
      restic = [
        (
          {
            targets = [ "${repo}" ];
            src = paths;
          }
          // extraOptions
        )
      ];

      path = mergedPaths;
    };

  # Merge many bk config items
  mergeBkConf =
    lst:
    builtins.foldl'
      (a: b: ({
        restic_target = a.restic_target // b.restic_target or { };
        path = a.path // b.path or { };
        restic = a.restic ++ b.restic or [ ];
        ntfy = a.ntfy // b.ntfy or { };
        restic_forget = a.restic_forget ++ b.restic_forget or [ ];
      }))
      {
        restic_target = { };
        path = { };
        restic = [ ];
        ntfy = { };
        restic_forget = [ ];
      }
      lst;
}
