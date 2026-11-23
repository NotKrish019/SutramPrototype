YUKTI - Digital Evidence Correlation & Crime Reconstruction
===========================================================

This archive contains two programs:

  yukti          the command-line workbench (run this)
  yukti-server   the local YUKTI API that yukti starts for you in the background

Start:

  yukti                  interactive session (type 'help')
  yukti doctor           check the installation
  yukti --help           every command

The local server keeps its data in ~/.yukti/data (Windows: %LOCALAPPDATA%\yukti\data)
and logs to ~/.yukti/server.log. Stop it with: yukti server stop

Use synthetic data only. Never load real victim or live case data into a development build.
