use std::vec::Vec;

use crate::base::{EncodingPacket, ObjectTransmissionInformation};
use crate::decoder::Decoder as DecoderNative;
use crate::encoder::Encoder as EncoderNative;
use pyo3::prelude::*;
use pyo3::types::*;

#[pyclass]
pub struct Encoder {
    encoder: EncoderNative,
}

#[pymethods]
impl Encoder {
    #[staticmethod]
    pub fn with_defaults(
        data: Bound<'_, PyBytes>,
        maximum_transmission_unit: u16,
    ) -> PyResult<Encoder> {
        let encoder = EncoderNative::with_defaults(data.as_bytes(), maximum_transmission_unit);
        Ok(Encoder { encoder })
    }

    pub fn get_encoded_packets(
        &self,
        py: Python<'_>,
        repair_packets_per_block: u32,
    ) -> PyResult<Vec<Py<PyBytes>>> {
        let packets: Vec<Py<PyBytes>> = self
            .encoder
            .get_encoded_packets(repair_packets_per_block)
            .iter()
            .map(|packet| PyBytes::new(py, &packet.serialize()).into())
            .collect();

        Ok(packets)
    }
}

#[pyclass]
pub struct Decoder {
    decoder: DecoderNative,
}

#[pymethods]
impl Decoder {
    #[staticmethod]
    pub fn with_defaults(
        transfer_length: u64,
        maximum_transmission_unit: u16,
    ) -> PyResult<Decoder> {
        let config = ObjectTransmissionInformation::with_defaults(
            transfer_length,
            maximum_transmission_unit,
        );
        let decoder = DecoderNative::new(config);
        Ok(Decoder { decoder })
    }

    pub fn decode(
        &mut self,
        py: Python<'_>,
        packet: Bound<'_, PyBytes>,
    ) -> PyResult<Option<Py<PyBytes>>> {
        let result = self
            .decoder
            .decode(EncodingPacket::deserialize(packet.as_bytes()));
        Ok(result.map(|data| PyBytes::new(py, &data).into()))
    }
}

#[pymodule]
pub fn raptorq(_py: Python<'_>, m: Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Encoder>()?;
    m.add_class::<Decoder>()?;
    Ok(())
}
