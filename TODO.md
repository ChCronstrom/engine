- ☑ Make search happen on separate thread, for compatibility with UCI GUIs
- ☑ Respond with "bestmove"
- ☑ Stop condition: stop command
- ☐ Stop condition: wtime btime winc binc time management
Engine should be compatible with GUIs at this point!

Additional stuff:
- ☑ Suitable hash table to prevent infinite growth
- ☑ Previous best move search heuristic
- ☑ Stop condition: depth parameter
- ☑ Stop condition: movetime parameter
- ☐ Stop condition: nodes parameter
- ☐ Stop condition: mate parameter
- ☐ setoption command
- ☐ Quiescent search: captures
- ☐ Quiescent search: check evasion
- ☐ Quiescent search: promotion
- ☐ Quiescent search: check opponent if interesting? (e.g. check with fork, smothered check, check with pawn advance)
- ☑ Report info: time, nodes, and and nps
- ☑ Reuse old data from hash table when search gets stopped
- ☑ Better purging strategy in hash map
- ☐ Crash on position fen r1b1k2r/pp3p2/5bp1/q1p4p/2PpP2P/PP6/R2PNnP1/1NQK1B1R w kq -
- ☐ Appears to play extremely weakly when search is aborted partway due to time constraint. Ignoring
    the search and going back to the previous depth appears to give stronger play, even if this throws
    away over half a minute's worth of thinking. Could be related to the hash table not purging entries
    in a reasonable way. Should maybe create a script for self-play to quantify this behaviour?
- ☐ Mark hash entries as belonging to this generation if they were useful in the search
- ☑ Accept the commands ucinewgame and isready
