#!/usr/bin/env bash

set -euo pipefail

project_dir="${1:-.}"

(
	cd "$project_dir"

	robo run python -c 'from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)'
	robo run env MPLBACKEND=QtAgg python -c 'import matplotlib.pyplot as plt; fig = plt.figure(); print(type(fig.canvas).__name__)'
)
