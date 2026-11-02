# SPDX-License-Identifier: GPL-3.0-or-later
import sys

if len(sys.argv) > 1 and sys.argv[1] == "smoke":
    from tools.play.smoke import main

    sys.exit(main(sys.argv[2:]))

from tools.play.cli import main

sys.exit(main())
