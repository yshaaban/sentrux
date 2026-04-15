function hasExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
}

function waitForExit(child, timeoutMs) {
  if (hasExited(child)) {
    return Promise.resolve(true);
  }

  return new Promise(function waitForChild(resolve) {
    const onExit = function onExit() {
      clearTimeout(timer);
      resolve(true);
    };
    const timer = setTimeout(function onTimeout() {
      child.off('exit', onExit);
      resolve(false);
    }, timeoutMs);
    child.once('exit', onExit);
  });
}

export async function closeChildProcess(child) {
  if (hasExited(child)) {
    return;
  }

  child.stdin.end();
  if (await waitForExit(child, 1000)) {
    return;
  }

  child.kill('SIGTERM');
  if (await waitForExit(child, 1000)) {
    return;
  }

  child.kill('SIGKILL');
  await waitForExit(child, 1000);
}
