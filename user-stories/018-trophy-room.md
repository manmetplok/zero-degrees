# Trophy Room — Badges

## Story
As a player, I want to earn trophies for feats like fast clears, calming angry customers, and finishing a course, so that good support habits are collected and shown off like achievements in a mobile game.

## Requirements

### Requirement: Trophy system
The system SHALL define trophies with clear earning conditions — including at least "Speed Demon" (10 clears under 5 minutes), "Firefighter" (douse 5 burning hurdles), "Peacekeeper" (clear 5 angry-aura hurdles), "Clean Sweep" (finish a day with an empty track), and "High Jumper" (clear 10 critical hurdles) — SHALL award each trophy automatically and only once, and SHALL display earned trophies in a trophy room on the player's profile and next to their league entry.

#### Scenario: Trophy earned
- **WHEN** a player meets a trophy's condition for the first time
- **THEN** the trophy is awarded with a full-screen celebration and appears in their trophy room

#### Scenario: No duplicates
- **WHEN** an earned trophy's condition is met again
- **THEN** no duplicate is awarded, but progress toward a tiered upgrade (bronze → silver → gold) may advance
