# Binoculars — Search and Filter

## Story
As a player, I want binoculars to scan the whole course — searching by text and filtering by channel, type, mood, height, and status — so that I can find a specific hurdle fast when a customer calls about it.

## Requirements

### Requirement: Course scanning
The system SHALL support free-text search over message content and sender, and SHALL support combinable filters for channel, category, sentiment, urgency, and open/cleared status, presented as a quick-access overlay on the track.

#### Scenario: Combined filters
- **WHEN** a player filters by channel "email" and type "billing"
- **THEN** only open email hurdles of type billing remain visible on the track

#### Scenario: Text search
- **WHEN** a player searches for a customer name or keyword
- **THEN** matching hurdles are listed with their scout reports, most relevant or most recent first, and tapping one jumps the camera to it
