import multiprocessing
import multiprocessing.queues
import queue
import time

from .logging import get_logger

from . import gpio


# =====
class Ps2Keyboard(multiprocessing.Process):
    def __init__(self, clock: int, data: int, pulse: float) -> None:
        super().__init__(daemon=True)

        self.__clock = gpio.set_output(clock, initial=True)
        self.__data = gpio.set_output(data, initial=True)
        self.__pulse = pulse

        self.__queue: multiprocessing.queues.Queue = multiprocessing.Queue()
        self.__event = multiprocessing.Event()

    def start(self) -> None:
        get_logger().info("Starting keyboard daemon ...")
        super().start()

    def stop(self) -> None:
        get_logger().info("Stopping keyboard daemon ...")
        self.__event.set()
        self.join()

    def send_byte(self, code: int) -> None:
        self.__queue.put(code)

    def cleanup(self) -> None:
        if self.is_alive():
            self.stop()

    def run(self) -> None:
        with gpio.bcm():
            try:
                while not self.__event.is_set():
                    try:
                        code = self.__queue.get(timeout=0.1)
                    except queue.Empty:
                        pass
                    else:
                        self.__send_byte(code)
            except Exception:
                get_logger().exception("Unhandled exception")
                raise

    def __send_byte(self, code: int) -> None:
        code_bits = list(map(bool, bin(code)[2:].zfill(8)))
        code_bits.reverse()
        message = [False] + code_bits + [(not sum(code_bits) % 2), True]
        for bit in message:
            self.__send_bit(bit)

    def __send_bit(self, bit: bool) -> None:
        gpio.write(self.__clock, True)
        gpio.write(self.__data, bool(bit))
        time.sleep(self.__pulse)
        gpio.write(self.__clock, False)
        time.sleep(self.__pulse)
        gpio.write(self.__clock, True)
