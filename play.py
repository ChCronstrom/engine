import subprocess
from subprocess import Popen, PIPE
from pychess.Utils import Move
from pychess.Utils.Board import Board
from pychess.Utils.const import DRAW_50MOVES, DRAW_INSUFFICIENT, DRAW_REPITITION, DRAW_STALEMATE, WON_CALLFLAG, WON_MATE
from pychess.Utils.const import DRAW, WHITEWON, BLACKWON
from pychess.Utils.const import WHITE, BLACK
from pychess.Utils.logic import getStatus
import time

ENCODING="utf-8"

def len_gt_zero(generator):
    for item in generator:
        return True

    return False

def get_walltime():
    # Milliseconds
    return time.monotonic_ns() // 1000_000

class Player:
    def __init__(self, engine=None):
        exec_name = engine if engine else "stockfish"
        process = Popen([exec_name], stdin=PIPE, stdout=PIPE)
        self._process = process
        self._pipe_send = process.stdin
        self._pipe_recv = process.stdout
        self._options = None
        self._midgameoptions = None
        self._maxdepth = None
        self._movetime = None
        self._time_management = True

        line = self._readline()

        self._sendline("uci")
        self._expectline("uciok")

    def set_option(self, name, value):
        if self._options == None:
            self._options = dict()

        self._options[name] = value

    def set_option_for_midgame(self, name, value):
        if self._midgameoptions == None:
            self._midgameoptions = dict()

        self._midgameoptions[name] = value

    def set_go_string(self, *, time_management: bool, maxdepth: int | None = None, movetime: int | None = None):
        self._time_management = time_management
        self._maxdepth = maxdepth
        self._movetime = movetime

    def new_game(self):
        self._sendline("ucinewgame")

        if self._options != None:
            for option, value in self._options.items():
                self._send_option(option, value)

        self._sendline("isready")
        isready = self._handleincoming()
        assert isready[0] == "readyok"

    def enter_midgame(self):
        if self._midgameoptions != None:
            for name, value in self._midgameoptions.items():
                self._send_option(name, value)

    def play_position(self, position, wtime=None, btime=None):
        self._searchinfo = dict()
        self._sendline("position " + position)
        go_string = "go"
        if self._maxdepth:
            go_string += f" depth {self._maxdepth}"
        if self._movetime:
            go_string += f" movetime {self._movetime}"
        if self._time_management:
            if wtime != None:
                go_string += f" wtime {wtime}"
            if btime != None:
                go_string += f" btime {btime}"
        self._sendline(go_string)
        result = self._handleincoming()
        assert result[0] == "bestmove"

        bestmove = result[1].split(" ", maxsplit=1)[0]

        return bestmove, self._searchinfo

    def close(self):
        self._sendline("quit")
        self._pipe_send.flush()
        self._process.wait()

    def _send_option(self, name, value):
        print(f"DEBUG: _send_option {name} = {value}    {self}")
        self._sendline("setoption name " + name + " value " + str(value))

    def _readline(self):
        while True:
            string = str(self._pipe_recv.readline(), encoding=ENCODING)
            string = string.strip()
            if len(string) > 0:
                return string


    def _sendline(self, line):
        self._pipe_send.write(bytes(line + "\n", encoding=ENCODING))

    def _expectline(self, expected):
        line = self._handleincoming()
        assert line[0] == expected, f"Expected {expected}, got {line}"

    def _handleincoming(self):
        self._pipe_send.flush()
        while True:
            line = self._readline()
            linewords = line.split(" ", maxsplit=1)
            if linewords[0] == "id":
                pass # Ignore
            elif linewords[0] == "info":
                infowords = linewords[1].split(" ")
                if "depth" in infowords:
                    depth = int(infowords[infowords.index("depth") + 1])
                    if "depth" not in self._searchinfo or depth > self._searchinfo["depth"]:
                        self._searchinfo["depth"] = depth
                if "score" in infowords:
                    location = infowords.index("score")
                    score = infowords[location + 1] + " " + infowords[location + 2]
                    self._searchinfo["score"] = score
                if "time" in infowords:
                    time_used = int(infowords[infowords.index("time") + 1])
                    if "time" not in self._searchinfo or time_used > self._searchinfo["time"]:
                        self._searchinfo["time"] = time_used
                if "tbhits" in infowords:
                    tbhits = int(infowords[infowords.index("tbhits") + 1])
                    if tbhits > 0 and ("tbhits" not in self._searchinfo or tbhits > self._searchinfo["tbhits"]):
                        self._searchinfo["tbhits"] = tbhits

            elif linewords[0] == "option":
                pass # Ignore
            else:
                return linewords

def decode_match_end(state: int, explanation: int) -> str:
    if state == DRAW:
        if explanation == DRAW_STALEMATE:
            result = "The game was a stalemate"
        else:
            result = "The game was a draw"
            if explanation == DRAW_50MOVES:
                result += " under the 50 moves rule"
            elif explanation == DRAW_REPITITION:
                result += " under the three repetitions rule"
            elif explanation == DRAW_INSUFFICIENT:
                result += " because there is insufficient material for a checkmate"
            else:
                result += f" with explanation {explanation}"

    else:
        if state == WHITEWON:
            result = "White won"
        elif state == BLACKWON:
            result = "Black won"

        if explanation == WON_MATE:
            result += " with a checkmate"
        else:
            result += f" with explanation {explanation}"

    return result



class Match:
    _players: tuple[Player, Player]
    _board: Board
    _time_left: list[int]
    _moves: list[Move.Move]
    _search_infos: list
    _position_string: str
    _status: tuple[int, int]

    def __init__(self, wplayer: Player, bplayer: Player, time: int=300_000, verbose: bool = False) -> None:
        self._time_left = [time, time]
        wplayer.new_game()
        bplayer.new_game()
        self._players = wplayer, bplayer
        self._board = Board(setup=True)
        self._moves = []
        self._search_infos = []
        self._position_string = "startpos moves"
        self._status = 0, 0
        self._verbose = verbose

    def play_ply(self):
        if self._board.ply == 18:
            self._players[WHITE].enter_midgame()
            self._players[BLACK].enter_midgame()

        color_up = self._board.color
        player_up: Player = self._players[color_up]
        position_up = self._position_string

        starttime = get_walltime()
        move_str, search_info = player_up.play_position(position_up, wtime=self._time_left[WHITE], btime=self._time_left[BLACK])
        stoptime = get_walltime()
        time_spent = stoptime - starttime

        move = Move.parseAN(self._board, move_str)
        self._time_left[color_up] -= time_spent

        self._position_string += (" " + move_str)
        self._moves.append(move)
        self._search_infos.append(search_info)

        if self._verbose:
            move_nbr = 1 + self._board.ply // 2
            san = Move.toSAN(self._board, move)
            if color_up == WHITE:
                print(f"{move_nbr:3}. {san:6}   ", end="")
            else:
                print(f"        {san:6}", end="")
                print(70*" ", end="")
            score = search_info["score"]
            depth = search_info["depth"]
            print(f"  time = {self._time_left[color_up] / 1000}", end="")
            for key, value in search_info.items():
                print(f"  {key} = {value:5}", end="")
            print("")

        # Update board
        self._board = self._board.move(move)

        # If time is up for color_up, that means self._board.color (which has now been updated) has won
        if False: #self._time_left[color_up] <= 0:
            self._status = (WHITEWON if self._board.color == WHITE else BLACKWON,
                            WON_CALLFLAG)
        else:
            self._status = getStatus(self._board)


    def finish_game(self) -> tuple[int, int]:
        while True:
            status, _ = self._status
            if status in [DRAW, WHITEWON, BLACKWON]:
                break

            self.play_ply()

        return self._status

def create_engine_player(engine="target/release/engine") -> Player:
    player = Player(engine)
    player.set_go_string(time_management=False, movetime=10000)
    return player

def create_engine_player_maxdepth(engine="target/release/engine") -> Player:
    player = Player(engine)
    player.set_go_string(time_management=False, maxdepth=8)
    return player

def create_stockfish20() -> Player:
    player = Player()
    player.set_option("Skill Level", 19)
    player.set_option_for_midgame("Skill Level", 20)
    return player

def create_stockfish19() -> Player:
    player = Player()
    player.set_option("Skill Level", 19)
    return player

def create_stockfish20_with_tables_5() -> Player:
    player = create_stockfish20()
    player.set_option("SyzygyPath", "/home/ccdl/Development/syzygy/download")
    return player

def create_stockfish20_with_tables_6() -> Player:
    player = create_stockfish20()
    player.set_option("SyzygyPath", "/home/ccdl/Development/syzygy/download6")
    return player

class Matchresult:
    def __init__(self, white, black):
        self.played = False
        self.white = white
        self.black = black
        self.result = None
        self.moves = 0

    def set_result(self, winner, moves):
        self.played = True
        self.result = winner
        self.moves = moves

class Teamresult:
    def __init__(self, name, matchresults: list[Matchresult]):
        self.name = name
        self.played = 0
        self.wins = 0
        self.draws = 0
        self.losses = 0
        self.points = 0
        self.points_as_black = 0
        self.movepoints = 0
        self.movepoints_as_black = 0

        for m in matchresults:
            if m.played:
                if m.white == name:
                    self.played += 1
                    if m.result == WHITEWON:
                        self.wins += 1
                        self.points += 2
                        self.movepoints -= m.moves
                    elif m.result == DRAW:
                        self.draws += 1
                        self.points += 1
                    elif m.result == BLACKWON:
                        self.losses += 1
                        self.movepoints += m.moves
                elif m.black == name:
                    self.played += 1
                    if m.result == BLACKWON:
                        self.wins += 1
                        self.points += 2
                        self.points_as_black += 2
                        self.movepoints -= m.moves
                        self.movepoints_as_black -= m.moves
                    elif m.result == DRAW:
                        self.draws += 1
                        self.points += 1
                        self.points_as_black += 1
                    elif m.result == WHITEWON:
                        self.losses += 1
                        self.movepoints += m.moves
                        self.movepoints_as_black += m.moves




def print_table(teams, matchresults):
    table = [Teamresult(team, matchresults) for team in teams]

    # Include sortkey and sort with criterion 1
    # 1. Rank by points earned in all matches
    # 2. then by number of matches still to play
    # 3. then by points earned as black
    table = [(item, (-item.points, -item.played, -item.points_as_black)) for item in table]
    table.sort(key=lambda x: x[1])

    print("Name        W  D  L  P")
    for t, _ in table:
        print(f"{t.name:9}: {t.wins:2} {t.draws:2} {t.losses:2}  {t.points/2}")


def main():
    #players = { "20": create_stockfish20(), "T5": create_stockfish20_with_tables_5(), "T6": create_stockfish20_with_tables_6(), "19": create_stockfish19() }
    # players = { "20": create_stockfish20(), "19": create_stockfish19() }
    players = { "new": create_engine_player(), "maxdepth": create_engine_player_maxdepth(), "old": create_engine_player("./oldengine") }
    matches = [ Matchresult(w, b) for w in players for b in players if w != b ]


    for matchentry in matches:
        print(f"Matching {matchentry.white} against {matchentry.black}")
        wplayer = players[matchentry.white]
        bplayer = players[matchentry.black]
        match = Match(wplayer, bplayer, time=4*60*1000, verbose=True)
        match.finish_game()
        print(match._board)
        print(decode_match_end(*match._status))

        matchentry.set_result(match._status[0], match._board.ply)
        print_table(list(players), matches)
        print("")



def two_players():
    wplayer = create_stockfish20()
    bplayer = create_stockfish19()

    match = Match(wplayer, bplayer, time=60000, verbose=True)
    match.finish_game()
    print(match._board)
    print(decode_match_end(*match._status))

    match = Match(bplayer, wplayer, time=60000, verbose=True)
    match.finish_game()
    print(match._board)
    print(decode_match_end(*match._status))

    wplayer.close()
    bplayer.close()

if __name__ == "__main__":
    main()
