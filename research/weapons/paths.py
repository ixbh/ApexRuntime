"""Folders the research scripts use, from the environment:

- IW4L_APEX_CONTENT: your personal Apex content folder (exports, captures);
  never inside a repo.
- R5_FLOWSTATE: your offline R5Flowstate install.
- IW4L_DIR: the IW4L checkout (default: `iw4l-apex` next to this repo).
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
CONTENT = os.environ.get("IW4L_APEX_CONTENT", r"F:\ApexContent")
FLOWSTATE = os.environ.get("R5_FLOWSTATE", r"F:\R5Flowstate")
IW4L = os.environ.get("IW4L_DIR", os.path.normpath(os.path.join(HERE, "..", "..", "..", "iw4l-apex")))
