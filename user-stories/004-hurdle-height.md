# Hurdle Height — AI Urgency Scoring

## Story
As a player, I want urgent messages to appear as taller, more dramatic hurdles worth more points, so that the most important work is literally the biggest thing on my screen.

## Requirements

### Requirement: Urgency sets height and reward
The system SHALL use an AI model to assign each message an urgency level (critical, high, normal, low), SHALL render urgency as hurdle height and visual intensity, and SHALL scale the hurdle's point reward with its height.

#### Scenario: Critical message towers over the track
- **WHEN** a message with time-critical content ("our service is down", "legal complaint") is ingested
- **THEN** it appears as a maximum-height hurdle with a warning glow and the highest point value

#### Scenario: Routine message is a low hop
- **WHEN** a routine informational message is ingested
- **THEN** it appears as a low hurdle with the base point value

#### Scenario: Height is honest
- **WHEN** a player opens any hurdle's detail card
- **THEN** the underlying urgency level and why-it-matters signal are shown, so the game reading never hides the real triage data
