import argparse
import logging
import logging.config

from typing import Dict

from .yaml import load_yaml_file


# =====
def init() -> Dict:
    parser = argparse.ArgumentParser()
    parser.add_argument("-c", "--config", required=True, metavar="<path>")
    options = parser.parse_args()

    config: Dict = load_yaml_file(options.config)

    logging.captureWarnings(True)
    logging.config.dictConfig(config["logging"])

    return config
