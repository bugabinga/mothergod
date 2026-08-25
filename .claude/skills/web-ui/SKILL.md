---
name: web-ui
description: "Use when changing mothergod's site, homepage, page layout, styling, browser interaction, or accessibility under site/."
user-invocable: true
---

# Web UI

Build the smallest honest static interface that answers one real user's
question.
Reuse the existing site before inventing another visual language.

## Sources

Read only what the screen needs:

- `site/` owns current implementation and visual vocabulary;
- `ROADMAP.md` owns mission and lifecycle;
- executable or generated evidence owns volatile product claims;
- `README.md` is existing public copy, not factual authority;
- `assets/logo.svg` is canonical and `site/logo.svg` is its deployment copy.

Read `.github/workflows/deploy-site.yml` only when deployment details matter.
Keep both logo files identical until a separate change removes the duplication.

## Procedure

1. Name one user, one question, and one representative screen.
2. Inspect the existing page, content evidence, and relevant deployment path.
3. Preserve the static, no-build architecture unless the task proves it
   insufficient.
4. Use semantic HTML, existing CSS custom properties, and minimal vanilla
   JavaScript only for real interaction.
5. Publish no volatile claim without naming its current evidence owner.
   Do not infer truth from existing public copy.
6. Keep dependencies at zero by default.
   Reuse an external asset only when the current site already owns that choice.
7. Make narrow and wide layouts explicit.
8. Test heading order, labels, keyboard navigation, visible focus, contrast,
   long text, missing data, and reduced motion where animation exists.
9. Verify links and assets from the deployed root.
10. Review the rendered screen, not only the source.

## Interface rules

- One obvious primary surface.
- State remains understandable without color alone.
- Actions use buttons; navigation uses links.
- Icon-only controls have accessible names.
- Text survives narrow screens and long values.
- Tables preserve labels and units.
- Current status remains legible without JavaScript.

## Completion

1. Run every CLAUDE.md pre-push gate.
2. Serve `site/` from the deployed root.
3. Render at 375x812 and 1440x900.
4. Exercise keyboard-only and no-JavaScript behavior where relevant.
5. Verify root-relative links and assets.
6. Record viewport and interaction evidence in the PR's verification section.
