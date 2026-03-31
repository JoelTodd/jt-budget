# Ratatui Frontend Style Guide

## Purpose

This document covers frontend implementation standards for a new Ratatui app.

It is intentionally limited to UI structure, rendering, interaction, and presentation. Product rules, persistence, sync, lifecycle, and general Rust conventions are documented elsewhere.

---

## 1. Frontend principles

- Build a clear, intentional interface.
- Prefer obvious interaction over novelty.
- Treat rendering as a projection of state.
- Design for terminal constraints from the start.

Make the UI consistently show:
- what the user is viewing
- what is interactive
- what is selected
- what is editable
- what just changed

---

## 2. Rendering conventions

- Keep one clear top-level render entrypoint.
- Compose downward: split regions, assign areas, render components.
- Keep render helpers narrow and single-purpose.
- Use Ratatui primitives for their intended jobs.

Use:
- `Layout` for structure
- `Block` for grouping
- `Paragraph` for prose
- `List` for linear collections
- `Table` for comparison
- `Tabs` for sibling sections
- `Clear` for overlays
- `Span`, `Line`, `Text` for structured text

Keep rendering code local, readable, and easy to follow.

---

## 3. Layout rules

- Prefer layout constraints over manual coordinate math.
- Design for smaller terminals as well as fullscreen.
- Keep important widths stable where practical.
- Use whitespace to create hierarchy.
- Use percentages selectively and deliberately.

When space gets tight, simplify in this order:
1. reduce decorative spacing
2. shorten labels
3. hide secondary metadata
4. collapse auxiliary regions
5. simplify dense views

Keep the primary task area usable at all times.

---

## 4. Text and content

- Use structured text rather than stitched strings.
- Write labels for quick scanning.
- Choose wrap vs truncate deliberately.
- Align numeric content consistently.
- Use Unicode where it improves clarity.

Prefer short, stable terms.  
Where content is truncated, provide a sensible place to see the full value.

---

## 5. Focus, selection, and editing

- Make focus visible at all times.
- Show focus, selection, and editing as distinct states.
- Make mode changes obvious.
- Give editing a distinct visual treatment.

Use multiple cues together:
- border treatment
- highlights
- cursor visibility
- titles
- local hints

This keeps interaction legible in a terminal environment.

---

## 6. Visual design and theming

- Define semantic style tokens.
- Use color to communicate meaning.
- Pair color with other visual cues.
- Use text modifiers sparingly and consistently.
- Theme for legibility across a range of terminal styles.

Define tokens such as:
- background
- panel background
- foreground
- muted foreground
- border
- active border
- accent
- selection
- success
- warning
- danger
- disabled

Keep styling decisions centralised and semantic.

---

## 7. Common interface patterns

### Lists
Use lists for linear choice. Keep selection obvious and metadata secondary.

### Tables
Use tables for comparison. Give each column a clear purpose and stable treatment.

### Detail views
Use detail views to expand the current selection without overcrowding the collection view.

### Forms
Make field order, focus, editing state, and validation easy to follow.

### Overlays and modals
Use overlays for short focused tasks. Clear the background region first and make dismissal obvious.

---

## 8. Input conventions

- Translate raw keys into semantic actions.
- Keep keyboard behavior consistent across the app.
- Make destructive actions feel deliberate.
- Use dedicated input widgets for real text entry.

Keep most UI code action-driven rather than key-event-driven.

---

## 9. Empty, loading, and error states

- Give every state an explicit presentation.
- Distinguish absence, loading, and failure clearly.
- Keep feedback local where possible.
- Write direct, practical state messages.

A good state message should show:
- what happened
- what area it affects
- whether the user can retry, continue, or back out

---

## 10. Motion and polish

- Use motion to support comprehension.
- Keep effects restrained and purposeful.
- Design so the UI still works perfectly without motion.

Use motion to reinforce:
- change
- progress
- temporary emphasis
- focused transitions

---

## 11. Resilience and edge cases

- Design for narrow and awkward states.
- Define fallback behavior explicitly.
- Keep the interface visually stable during change.

Plan in advance:
- what hides first
- what shortens first
- what must remain visible
- how dense views simplify

---

## 12. Custom widgets

- Extract repeated presentation patterns into reusable widgets.
- Keep custom widgets single-purpose.
- Give custom widgets semantic inputs.

Good candidates:
- labeled value rows
- status pills
- section headers
- summary strips
- repeated row formats

This keeps rendering code smaller and more consistent.

---

## 13. Frontend quality bar

Build for an interface that feels:

- deliberate
- readable
- stable
- predictable
- restrained
- keyboard-native
- honest about state

The goal is a Ratatui frontend that feels composed and trustworthy under normal terminal constraints.
