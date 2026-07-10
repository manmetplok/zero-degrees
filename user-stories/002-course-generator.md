# Course Generator — AI Sample Data

## Story
As a developer demoing the game, I want an AI-generated demo course of realistic sample messages, so that we can play a full run live without touching real customer data.

## Requirements

### Requirement: AI-generated demo course
The system SHALL provide a seed mechanism that uses AI to generate a varied course of at least 50 sample messages (mixed channels, topics, tones, urgency, languages) and SHALL NOT include any real customer data.

#### Scenario: Building a course
- **WHEN** the course generator is run
- **THEN** the track is populated with hurdles of visibly different types, heights, and moods

#### Scenario: Fresh course
- **WHEN** the generator is run with a reset flag
- **THEN** the old course is cleared and a new one is laid out
- **AND** difficulty presets (chill jog / normal shift / nightmare Monday) change the mix of urgency and sentiment
