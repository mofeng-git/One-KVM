//! Safe wrapper for the `VIDIOC_SUBSCRIBE_EVENT` and `VIDIOC_UNSUBSCRIBE_EVENT
//! ioctls.

use std::convert::TryFrom;
use std::convert::TryInto;
use std::os::unix::io::AsRawFd;

use bitflags::bitflags;
use nix::errno::Errno;
use thiserror::Error;

use crate::bindings;
use crate::bindings::v4l2_event;
use crate::bindings::v4l2_event_subscription;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SubscribeEventFlags: u32 {
        const SEND_INITIAL = bindings::V4L2_EVENT_SUB_FL_SEND_INITIAL;
        const ALLOW_FEEDBACK = bindings::V4L2_EVENT_SUB_FL_ALLOW_FEEDBACK;
    }

}

#[derive(Debug)]
pub enum EventType {
    VSync,
    Eos,
    Ctrl(u32),
    FrameSync,
    SourceChange(u32),
    MotionDet,
}

#[derive(Debug, Error)]
pub enum EventConversionError {
    #[error("unrecognized event {0}")]
    UnrecognizedEvent(u32),
    #[error("unrecognized source change {0}")]
    UnrecognizedSourceChange(u32),
}

impl TryFrom<&v4l2_event_subscription> for EventType {
    type Error = EventConversionError;

    fn try_from(event: &v4l2_event_subscription) -> Result<Self, Self::Error> {
        Ok(match event.type_ {
            bindings::V4L2_EVENT_VSYNC => EventType::VSync,
            bindings::V4L2_EVENT_EOS => EventType::Eos,
            bindings::V4L2_EVENT_CTRL => EventType::Ctrl(event.id),
            bindings::V4L2_EVENT_FRAME_SYNC => EventType::FrameSync,
            bindings::V4L2_EVENT_SOURCE_CHANGE => EventType::SourceChange(event.id),
            bindings::V4L2_EVENT_MOTION_DET => EventType::MotionDet,
            e => return Err(EventConversionError::UnrecognizedEvent(e)),
        })
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SrcChanges: u32 {
        const RESOLUTION = bindings::V4L2_EVENT_SRC_CH_RESOLUTION;
    }
}

#[derive(Debug)]
pub enum Event {
    SrcChangeEvent(SrcChanges),
    Eos,
}

impl TryFrom<v4l2_event> for Event {
    type Error = EventConversionError;

    fn try_from(value: v4l2_event) -> Result<Self, Self::Error> {
        Ok(match value.type_ {
            bindings::V4L2_EVENT_VSYNC => todo!(),
            bindings::V4L2_EVENT_EOS => Event::Eos,
            bindings::V4L2_EVENT_CTRL => todo!(),
            bindings::V4L2_EVENT_FRAME_SYNC => todo!(),
            bindings::V4L2_EVENT_SOURCE_CHANGE => {
                let changes = unsafe { value.u.src_change.changes };
                Event::SrcChangeEvent(
                    SrcChanges::from_bits(changes)
                        .ok_or(EventConversionError::UnrecognizedSourceChange(changes))?,
                )
            }
            bindings::V4L2_EVENT_MOTION_DET => todo!(),
            t => return Err(EventConversionError::UnrecognizedEvent(t)),
        })
    }
}

fn build_v4l2_event_subscription(
    event: EventType,
    flags: SubscribeEventFlags,
) -> v4l2_event_subscription {
    v4l2_event_subscription {
        type_: match event {
            EventType::VSync => bindings::V4L2_EVENT_VSYNC,
            EventType::Eos => bindings::V4L2_EVENT_EOS,
            EventType::Ctrl(_) => bindings::V4L2_EVENT_CTRL,
            EventType::FrameSync => bindings::V4L2_EVENT_FRAME_SYNC,
            EventType::SourceChange(_) => bindings::V4L2_EVENT_SOURCE_CHANGE,
            EventType::MotionDet => bindings::V4L2_EVENT_MOTION_DET,
        },
        id: match event {
            EventType::Ctrl(id) => id,
            EventType::SourceChange(id) => id,
            _ => 0,
        },
        flags: flags.bits(),
        ..Default::default()
    }
}

#[doc(hidden)]
mod ioctl {
    use crate::bindings::{v4l2_event, v4l2_event_subscription};

    nix::ioctl_read!(vidioc_dqevent, b'V', 89, v4l2_event);
    nix::ioctl_write_ptr!(vidioc_subscribe_event, b'V', 90, v4l2_event_subscription);
    nix::ioctl_write_ptr!(vidioc_unsubscribe_event, b'V', 91, v4l2_event_subscription);
}

#[derive(Debug, Error)]
pub enum SubscribeEventError {
    #[error("ioctl error: {0}")]
    IoctlError(#[from] Errno),
}

impl From<SubscribeEventError> for Errno {
    fn from(err: SubscribeEventError) -> Self {
        match err {
            SubscribeEventError::IoctlError(e) => e,
        }
    }
}

/// Safe wrapper around the `VIDIOC_SUBSCRIBE_EVENT` ioctl.
pub fn subscribe_event(
    fd: &impl AsRawFd,
    event: EventType,
    flags: SubscribeEventFlags,
) -> Result<(), SubscribeEventError> {
    let subscription = build_v4l2_event_subscription(event, flags);

    unsafe { ioctl::vidioc_subscribe_event(fd.as_raw_fd(), &subscription) }?;
    Ok(())
}

/// Safe wrapper around the `VIDIOC_UNSUBSCRIBE_EVENT` ioctl.
pub fn unsubscribe_event(fd: &impl AsRawFd, event: EventType) -> Result<(), SubscribeEventError> {
    let subscription = build_v4l2_event_subscription(event, SubscribeEventFlags::empty());

    unsafe { ioctl::vidioc_unsubscribe_event(fd.as_raw_fd(), &subscription) }?;
    Ok(())
}

/// Safe wrapper around the `VIDIOC_UNSUBSCRIBE_EVENT` ioctl to unsubscribe from all events.
pub fn unsubscribe_all_events(fd: &impl AsRawFd) -> Result<(), SubscribeEventError> {
    let subscription = v4l2_event_subscription {
        type_: bindings::V4L2_EVENT_ALL,
        ..Default::default()
    };

    unsafe { ioctl::vidioc_unsubscribe_event(fd.as_raw_fd(), &subscription) }?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum DqEventError {
    #[error("no event ready for dequeue")]
    NotReady,
    #[error("error while converting event")]
    EventConversionError,
    #[error("unexpected ioctl error: {0}")]
    IoctlError(Errno),
}

impl From<Errno> for DqEventError {
    fn from(error: Errno) -> Self {
        match error {
            Errno::ENOENT => Self::NotReady,
            error => Self::IoctlError(error),
        }
    }
}

impl From<DqEventError> for Errno {
    fn from(err: DqEventError) -> Self {
        match err {
            DqEventError::NotReady => Errno::ENOENT,
            DqEventError::EventConversionError => Errno::EINVAL,
            DqEventError::IoctlError(e) => e,
        }
    }
}

pub fn dqevent<O: TryFrom<v4l2_event>>(fd: &impl AsRawFd) -> Result<O, DqEventError> {
    let mut event: v4l2_event = Default::default();

    match unsafe { ioctl::vidioc_dqevent(fd.as_raw_fd(), &mut event) } {
        Ok(_) => Ok(event
            .try_into()
            .map_err(|_| DqEventError::EventConversionError)?),
        Err(Errno::ENOENT) => Err(DqEventError::NotReady),
        Err(e) => Err(DqEventError::IoctlError(e)),
    }
}
