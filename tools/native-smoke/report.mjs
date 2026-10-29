// Reject callback-only, stale-host and unusable-window reports.
export function validateNativeFrame(frame, host = process) {
  const platform = { darwin: 'macos', win32: 'windows', linux: 'linux' }[host.platform];
  const arch = { arm64: 'aarch64', x64: 'x86_64' }[host.arch];
  if (!platform || !arch || !frame || frame.platform !== platform || frame.arch !== arch ||
      frame.renderedFrame !== true || !Number.isSafeInteger(frame.submittedFrames) || frame.submittedFrames < 1 ||
      !Number.isFinite(frame.width) || !Number.isFinite(frame.height) || frame.width < 960 || frame.height < 600) {
    throw new Error('Invalid native frame report (requires matching host and a submitted frame)');
  }
  return { ...frame, status: 'passed', physicalDesktopAcceptance: 'not_evaluated' };
}
