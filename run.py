import sys

from admin import ensure_daemon
from helpers import *


def main():
    if sys.stdin.isatty():
        sys.exit("bh reads Python from stdin. Use:\n  bh <<'PY'\n  print(page_info())\n  PY")
    ensure_daemon()
    exec(sys.stdin.read())


if __name__ == "__main__":
    main()
