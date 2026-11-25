import json
import multiprocessing
import sys
from quickclip321.acceptance import destination_acceptance

if __name__ == "__main__":
    multiprocessing.freeze_support()
    print(json.dumps(destination_acceptance(sys.argv[1]), indent=2))
