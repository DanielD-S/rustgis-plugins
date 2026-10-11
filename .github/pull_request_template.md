## Plugin

- Id:
- What it does:
- Source code (for a plugin hosted elsewhere):

## Checklist

- [ ] `registry/<id>.json` is named after the plugin id
- [ ] `python scripts/build.py --check` passes
- [ ] The plugin asks only for the permissions it uses (`edit`, `files`), and its description says why
- [ ] A new version has a new `version` in `plugin.json` (published packages never change)
