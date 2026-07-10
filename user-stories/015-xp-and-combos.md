# XP and Combos

## Story
As a player, I want XP for every cleared hurdle with combo multipliers for consecutive clears, so that getting into a flow on the queue feels as good as a combo streak in a mobile game.

## Requirements

### Requirement: XP scoring with combos
The system SHALL award XP on every hurdle clear scaled by hurdle height (urgency), SHALL apply a combo multiplier that grows with consecutive clears inside a time window and resets when the window lapses, and SHALL show XP and the live combo meter during play.

#### Scenario: Base clear
- **WHEN** a player clears a low hurdle
- **THEN** base XP is awarded immediately with a score pop-up

#### Scenario: Combo builds
- **WHEN** a player clears three hurdles within the combo window
- **THEN** the multiplier increases each clear (e.g. ×1 → ×1.5 → ×2) and the combo meter shows time remaining

#### Scenario: Combo breaks
- **WHEN** the combo window lapses without a clear
- **THEN** the multiplier resets to ×1 without deducting earned XP

#### Scenario: Big air
- **WHEN** a player clears a critical hurdle within its response target
- **THEN** height and speed bonuses stack with the current combo multiplier and a celebration animation plays
