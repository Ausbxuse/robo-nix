#!/usr/bin/env bash

set -euo pipefail

project_dir="${1:-.}"

(
	cd "$project_dir"

	robo run python -c 'from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)'
	robo run python -c 'import matplotlib; import matplotlib.pyplot as plt; fig = plt.figure(); print(matplotlib.get_backend()); assert type(fig.canvas).__name__ != "FigureCanvasAgg"'
)
