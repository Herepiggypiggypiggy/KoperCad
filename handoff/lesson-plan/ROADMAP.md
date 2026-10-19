# Lesson plan and learning menus: roadmap

Scope: the structured lesson plan (six tracks, order and `needs` for every lesson, `early` status), the
Learn window and Help menu structure, and coordination of the lesson-author threads, the Learning runner
thread and the reviewers. Design: `docs/design/lesson-plan.md` (section 5 lists the PRs).

## Done
- PR 1 plan, tracks, order, needs, `early` status, Early access badge (#583)
- Cabinet lesson anchors (#658)
- PR 2 Learn window: Continue card, track progress and minutes, "Do first" links (#674)
- PR 3 Help menu: "Next lesson: <title>" (`help.next_lesson`), Learning paths with `done/total`
  (opens the window at one track), card prefers the running lesson

## Next, in order
1. PR 4 Paths (`paths.toml`) at the top of the Learn window: in review (PR 958 is merged)
2. PR 5 "Lesson:" chips on panels (terminals, cables, wire list, cabinet drawers)
3. PR 6 Ctrl+K finds lessons by title and topic (`Target::Lesson`, `LessonEntry`): in review
4. Start panel line "Start with: First hour" / "Continue: <lesson>"
5. Promote lessons from `early` to `ready` as their authors close the GAPs

## Open questions
- Which lessons keep real GAPs (NOTES.md in lesson folders) and stay `early`.
- Advanced level has no lesson yet.
