# TUI Colour Swarm Review

Date: 2026-04-01 14:48:49 BST

## Scope

Surface-only review of the live TUI at `80x24`, `105x48`, and `210x48`, covering navigation, guided creation, and the monthly sheet. This report consolidates three tastemaker passes and removes duplicate points.

## Consolidated Findings

### 1. Active selection still sits too close to the base neutral
Where: Navigation, especially with one selected month and one unselected month visible.

What happens: The selected row is brighter, but it still lives in the same muted family as the rest of the table. State, diff, and timestamp can compete with the row highlight instead of supporting it.

Why it matters: Keyboard focus is not instantly obvious on a quick scan. The user has to parse row content before they feel anchored.

Suggestion: Give the selected row a clearer accent identity, then push unselected rows a step quieter so the active month reads first.

### 2. Guided creation does not give the current step enough colour ownership
Where: Guided creation at all sizes, most noticeably in `80x24` and `105x48`.

What happens: The active step, helper copy, next-step hint, and preview totals all sit too close together in tone. The screen changes structurally as the user advances, but not enough chromatically.

Why it matters: The flow feels flatter than it should. Colour is not doing enough to answer "what am I editing right now?" versus "what is just context?"

Suggestion: Reserve the clearest accent for the current step or active value, keep helper copy quieter, and let the preview totals sit between those two levels.

### 3. Validation and budget outcome are not visually dominant enough
Where: Guided preview and monthly-sheet footer.

What happens: Invalidity, persistence, sync, and draft state all carry similar visual weight. The user can read the words, but the colour hierarchy does not clearly rank the budget result above the operational metadata.

Why it matters: The main question in this app is whether the month balances. Colour should answer that before it answers whether autosave is clean or sync is fine.

Suggestion: Make validity and overall difference the strongest semantic cue, keep draft a quieter warning, and reduce persistence/sync to clearly secondary status tones.

### 4. Monthly-sheet sections are still too uniform to build a fast mental map
Where: Monthly sheet, especially `210x48`.

What happens: Accounts, Timing, Earmarks, Pots, and Validation use restrained styling, but the section rhythm is still too even. On wide screens the page reads as repeated bands rather than clearly distinct zones.

Why it matters: The user edits by section and re-scans frequently. If each area feels tonally similar, colour stops helping with re-orientation.

Suggestion: Keep the palette restrained, but give each major section a subtle, stable semantic tint in its heading or border so the sheet is easier to re-find by eye.

### 5. Everyday negative cues and true failure states borrow too much from the same danger family
Where: Accounts, differences, and validation.

What happens: Liability markers, negative amounts, and invalid totals all lean warm enough to feel related. The system is coherent, but it slightly overstates normal deduction cues by borrowing from the same emotional register as actual failure.

Why it matters: The user needs to distinguish "this value is negative by design" from "this month is out of tolerance." If both feel similarly dangerous, the semantics blur.

Suggestion: Keep invalidity as the strongest danger tone and soften routine liability or minus cues toward a calmer semantic accent.

### 6. The wide layout still leaves colour doing less work than the available space allows
Where: Navigation and monthly sheet at `210x48`.

What happens: Extra width mostly produces larger neutral regions. The interface stays tasteful, but wide layouts do not gain proportionate hierarchy from colour.

Why it matters: On a wide terminal, whitespace alone does not create enough scan rhythm. Subtle colour differences become more important, not less.

Suggestion: Use slightly stronger background or border separation between major regions in the wide profile so the added space feels intentionally structured rather than just expanded.

## Overall Direction

The palette is disciplined and already more coherent than the older monochrome treatment, but it still pulls back in exactly the places where colour could most improve clarity: active focus, guided-step ownership, validation priority, and wide-screen section mapping. The next pass should strengthen those semantic signals without making the app louder.
