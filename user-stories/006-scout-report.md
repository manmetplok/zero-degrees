# Scout Report — AI Summaries

## Story
As a player approaching a hurdle, I want a one-line AI scout report telling me what this hurdle is about, so that I can size up the jump without reading a wall of text mid-run.

## Requirements

### Requirement: Scout report card
The system SHALL generate a one-to-two-sentence AI summary per message and SHALL display it on the hurdle's approach card, with the full original message one tap deeper.

#### Scenario: Long message, short report
- **WHEN** a player taps a hurdle backed by a multi-paragraph message
- **THEN** the approach card leads with the AI summary
- **AND** a "read full message" control reveals the complete original text

#### Scenario: Short message
- **WHEN** the message is already shorter than the summary threshold
- **THEN** the original text is shown directly and no model call is made
