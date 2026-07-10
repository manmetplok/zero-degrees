# Hurdle Types — AI Categorization

## Story
As a player, I want the AI to give every hurdle a type based on the message's topic (billing, complaint, question, feedback), so that I can read the track at a glance and pick my line like in a real race.

## Requirements

### Requirement: AI-typed hurdles
The system SHALL call an AI model at ingestion to assign each message one category from a configurable set, and SHALL render that category as the hurdle's visual type (shape, color, icon) on the track.

#### Scenario: New hurdle gets a type
- **WHEN** a new message is ingested
- **THEN** the AI classifies it
- **AND** the hurdle appears with the matching type visuals (e.g. a red "complaint" barrier, a blue "question" gate)

#### Scenario: Retyping a hurdle
- **WHEN** a player corrects a hurdle's category from the detail card
- **THEN** the manual category overrides the AI's, the hurdle re-skins instantly, and the override is persisted
