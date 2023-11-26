# Reliable serial communication project
Kim Kuparinen, Ville Heikkinen, Aleksi Jarva, Jannatul Nilanti

## Serial communication
- Serializing and deserializing messages works.
- Implemented Hamming code to fix one bit errors and responses with Recovered status.
- Detects errors which have more than one bit flipped and responses with NotOk status.

## Host program
- CLI application to send messages to the ESP
- If the host program is started before the ESP the first serial message read by the host is not valid and causes a host panic. This is not wanted behaviour and should be fixed.

## ESP features
- RGB led can be turned on/off and color is decided by the current time on the board.
- Current time can be set
- Blink task can be set, either to start now or at given UTC time in the future. Frequency and duration can be set.
- Time is stored in time stamp representing seconds meaning that blink task can be controlled with accuracy of a second.
- When using rtc-timer with our implementation of time tracking the time started drifting very quickly. If instead of SystemTimer was used the drifting was not a problem. 
