package com.phantom.server;

import android.os.SystemClock;
import android.view.InputDevice;
import android.view.InputEvent;
import android.view.MotionEvent;

import java.io.BufferedInputStream;
import java.io.EOFException;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Method;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;

public final class PhantomServer {
    private static final int MAX_SLOTS = 10;

    private static final int CMD_TOUCH_DOWN = 0x00;
    private static final int CMD_TOUCH_MOVE = 0x01;
    private static final int CMD_TOUCH_UP = 0x02;
    private static final int CMD_TOUCH_CANCEL = 0x03;
    private static final int CMD_PING = 0x7f;

    private final String bindHost;
    private final int port;
    private final TouchInjector injector = new TouchInjector();

    private PhantomServer(String bindHost, int port) {
        this.bindHost = bindHost;
        this.port = port;
    }

    public static void main(String[] args) throws Exception {
        String bindHost = "0.0.0.0";
        int port = 27183;
        for (int i = 0; i < args.length; i++) {
            if ("--host".equals(args[i]) && i + 1 < args.length) {
                bindHost = args[++i];
            } else if ("--port".equals(args[i]) && i + 1 < args.length) {
                port = Integer.parseInt(args[++i]);
            }
        }

        System.out.println("phantom-server starting host=" + bindHost + " port=" + port);
        new PhantomServer(bindHost, port).run();
    }

    private void run() throws Exception {
        try (ServerSocket server = new ServerSocket()) {
            server.setReuseAddress(true);
            server.bind(new InetSocketAddress(InetAddress.getByName(bindHost), port), 1);
            while (true) {
                try (Socket client = server.accept()) {
                    client.setTcpNoDelay(true);
                    System.out.println("phantom-server client connected");
                    runSession(client.getInputStream(), client.getOutputStream());
                    injector.cancelAll();
                    System.out.println("phantom-server client disconnected");
                } catch (EOFException eof) {
                    injector.cancelAll();
                    System.out.println("phantom-server client closed");
                }
            }
        }
    }

    private void runSession(InputStream clientIn, OutputStream out) throws Exception {
        InputStream rawIn = new BufferedInputStream(clientIn);

        while (true) {
            int kind = rawIn.read();
            if (kind == -1) {
                throw new EOFException("socket closed");
            }

            switch (kind) {
                case CMD_TOUCH_DOWN:
                    injector.touchDown(readUnsignedByte(rawIn), readIntLe(rawIn), readIntLe(rawIn));
                    break;
                case CMD_TOUCH_MOVE:
                    injector.touchMove(readUnsignedByte(rawIn), readIntLe(rawIn), readIntLe(rawIn));
                    break;
                case CMD_TOUCH_UP:
                    injector.touchUp(readUnsignedByte(rawIn));
                    break;
                case CMD_TOUCH_CANCEL:
                    injector.cancelAll();
                    break;
                case CMD_PING:
                    out.write(CMD_PING);
                    out.flush();
                    break;
                default:
                    throw new IOException("unknown command byte " + kind);
            }
        }
    }

    private static int readUnsignedByte(InputStream in) throws IOException {
        int value = in.read();
        if (value == -1) {
            throw new EOFException("unexpected end of stream");
        }
        return value & 0xff;
    }

    private static int readIntLe(InputStream in) throws IOException {
        byte[] bytes = new byte[4];
        readFully(in, bytes);
        return (bytes[0] & 0xff)
                | ((bytes[1] & 0xff) << 8)
                | ((bytes[2] & 0xff) << 16)
                | ((bytes[3] & 0xff) << 24);
    }

    private static void readFully(InputStream in, byte[] buffer) throws IOException {
        int offset = 0;
        while (offset < buffer.length) {
            int read = in.read(buffer, offset, buffer.length - offset);
            if (read == -1) {
                throw new EOFException("unexpected end of stream");
            }
            offset += read;
        }
    }

    private static final class TouchInjector {
        private final PointerState[] states = new PointerState[MAX_SLOTS];
        private final MotionEvent.PointerProperties[] pointerProperties =
                new MotionEvent.PointerProperties[MAX_SLOTS];
        private final MotionEvent.PointerCoords[] pointerCoords =
                new MotionEvent.PointerCoords[MAX_SLOTS];
        private long downTime;

        private TouchInjector() {
            for (int i = 0; i < MAX_SLOTS; i++) {
                states[i] = new PointerState();
                pointerProperties[i] = new MotionEvent.PointerProperties();
                pointerCoords[i] = new MotionEvent.PointerCoords();
            }
        }

        void touchDown(int slot, int x, int y) throws Exception {
            if (!isValidSlot(slot)) {
                return;
            }

            if (states[slot].active) {
                touchMove(slot, x, y);
                return;
            }

            long now = SystemClock.uptimeMillis();
            int before = activeCount();
            if (before == 0) {
                downTime = now;
            }

            PointerState state = states[slot];
            state.active = true;
            state.x = x;
            state.y = y;

            int after = activeCount();
            int action = after == 1
                    ? MotionEvent.ACTION_DOWN
                    : MotionEvent.ACTION_POINTER_DOWN
                    | (activeIndex(slot) << MotionEvent.ACTION_POINTER_INDEX_SHIFT);
            injectCurrentPointers(action, now);
        }

        void touchMove(int slot, int x, int y) throws Exception {
            if (!isValidSlot(slot) || !states[slot].active) {
                return;
            }

            PointerState state = states[slot];
            state.x = x;
            state.y = y;
            injectCurrentPointers(MotionEvent.ACTION_MOVE, SystemClock.uptimeMillis());
        }

        void touchUp(int slot) throws Exception {
            if (!isValidSlot(slot) || !states[slot].active) {
                return;
            }

            long now = SystemClock.uptimeMillis();
            int count = activeCount();
            int action = count == 1
                    ? MotionEvent.ACTION_UP
                    : MotionEvent.ACTION_POINTER_UP
                    | (activeIndex(slot) << MotionEvent.ACTION_POINTER_INDEX_SHIFT);

            injectCurrentPointers(action, now);
            states[slot].active = false;
            if (count == 1) {
                downTime = 0L;
            }
        }

        void cancelAll() throws Exception {
            if (activeCount() == 0) {
                return;
            }

            injectCurrentPointers(MotionEvent.ACTION_CANCEL, SystemClock.uptimeMillis());
            for (PointerState state : states) {
                state.active = false;
            }
            downTime = 0L;
        }

        private void injectCurrentPointers(int action, long eventTime) throws Exception {
            int pointerCount = fillActivePointers();
            if (pointerCount == 0) {
                return;
            }

            MotionEvent event = MotionEvent.obtain(
                    downTime == 0L ? eventTime : downTime,
                    eventTime,
                    action,
                    pointerCount,
                    pointerProperties,
                    pointerCoords,
                    0,
                    0,
                    1f,
                    1f,
                    0,
                    0,
                    InputDevice.SOURCE_TOUCHSCREEN,
                    0
            );

            try {
                InputManagerFacade.inject(event);
            } finally {
                event.recycle();
            }
        }

        private int fillActivePointers() {
            int index = 0;
            for (int slot = 0; slot < MAX_SLOTS; slot++) {
                PointerState state = states[slot];
                if (!state.active) {
                    continue;
                }

                MotionEvent.PointerProperties properties = pointerProperties[index];
                properties.clear();
                properties.id = slot;
                properties.toolType = MotionEvent.TOOL_TYPE_FINGER;

                MotionEvent.PointerCoords coords = pointerCoords[index];
                coords.clear();
                coords.x = state.x;
                coords.y = state.y;
                coords.pressure = 1f;
                coords.size = 1f;
                coords.touchMajor = 1f;

                index++;
            }
            return index;
        }

        private int activeCount() {
            int count = 0;
            for (PointerState state : states) {
                if (state.active) {
                    count++;
                }
            }
            return count;
        }

        private int activeIndex(int slot) {
            int index = 0;
            for (int i = 0; i < MAX_SLOTS; i++) {
                if (!states[i].active) {
                    continue;
                }
                if (i == slot) {
                    return index;
                }
                index++;
            }
            return 0;
        }

        private boolean isValidSlot(int slot) {
            return slot >= 0 && slot < MAX_SLOTS;
        }
    }

    private static final class PointerState {
        boolean active;
        float x;
        float y;
    }

    private static final class InputManagerFacade {
        private static final int INJECT_INPUT_EVENT_MODE_ASYNC = 0;
        private static final Object INPUT_MANAGER;
        private static final Method INJECT_INPUT_EVENT;

        static {
            try {
                Class<?> inputManagerClass =
                        Class.forName("android.hardware.input.InputManager");
                Method getInstance = inputManagerClass.getDeclaredMethod("getInstance");
                getInstance.setAccessible(true);
                INPUT_MANAGER = getInstance.invoke(null);
                INJECT_INPUT_EVENT = inputManagerClass.getMethod(
                        "injectInputEvent",
                        InputEvent.class,
                        int.class
                );
                INJECT_INPUT_EVENT.setAccessible(true);
            } catch (Exception e) {
                throw new RuntimeException("failed to initialize InputManager reflection", e);
            }
        }

        static void inject(InputEvent event) throws Exception {
            boolean ok = (Boolean) INJECT_INPUT_EVENT.invoke(
                    INPUT_MANAGER,
                    event,
                    INJECT_INPUT_EVENT_MODE_ASYNC
            );
            if (!ok) {
                throw new IOException("injectInputEvent returned false");
            }
        }
    }
}
