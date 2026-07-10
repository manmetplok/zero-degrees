# Mood Auras — AI Sentiment

## Story
As a player, I want each hurdle to carry a mood aura from AI sentiment analysis (flames for angry, rain cloud for sad, sparkles for happy), so that I know what emotional state I'm walking into before I take the jump.

## Requirements

### Requirement: Sentiment as aura
The system SHALL classify each message's sentiment (positive, neutral, negative, angry) with an AI model and SHALL render it as a visual aura effect on the hurdle, visible from the track without opening the message.

#### Scenario: Angry customer burns
- **WHEN** a message with angry content is ingested
- **THEN** its hurdle gets a flame aura
- **AND** clearing it well earns a bonus (see badges: "Firefighter")

#### Scenario: Filtering by mood
- **WHEN** a player filters the track by aura
- **THEN** only hurdles with the selected sentiment remain highlighted, the rest dim
