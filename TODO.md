- ☑ Make search happen on separate thread, for compatibility with UCI GUIs
- ☑ Respond with "bestmove"
- ☑ Stop condition: stop command
- ☐ Stop condition: wtime btime winc binc time management
Engine should be compatible with GUIs at this point!

Additional stuff:
- ☑ Suitable hash table to prevent infinite growth
- ☐ Previous best move search heuristic
- ☑ Stop condition: depth parameter
- ☐ Stop condition: movetime parameter
- ☐ Stop condition: nodes parameter
- ☐ Stop condition: mate parameter
- ☐ setoption command
- ☐ Quiescent search: captures
- ☐ Quiescent search: check evasion
- ☐ Quiescent search: promotion
- ☐ Quiescent search: check opponent if interesting? (e.g. check with fork, smothered check, check with pawn advance)
- ☑ Report info: time, nodes, and and nps
- ☑ Reuse old data from hash table when search gets stopped
- ☐ Better purging strategy in hash map
