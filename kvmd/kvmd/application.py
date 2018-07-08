import argparse
import logging
import logging.config

from typing import Dict

import yaml


# =====
def init() -> Dict:
    parser = argparse.ArgumentParser()
    parser.add_argument("-c", "--config", required=True, metavar="<path>")
    options = parser.parse_args()

    with open(options.config) as config_file:
        config = yaml.load(config_file)

    logging.captureWarnings(True)
    logging.config.dictConfig(config["logging"])

    return config["kvmd"]
