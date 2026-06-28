package m1;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

import sun.misc.Unsafe;

public class BytecodeFeatures {
    private int stored;

    public int arithmetic(int left, int right) {
        int sum = left + right;
        return sum * 2;
    }

    public int fieldAccess(int value) {
        this.stored = value;
        return this.stored;
    }

    public int choose(int value) {
        switch (value) {
            case 1:
                return 10;
            case 2:
                return 20;
            default:
                return 30;
        }
    }

    public synchronized void synchronizedMethod() {
    }

    public void synchronizedBlock(Object lock) {
        synchronized (lock) {
            lock.toString();
        }
    }

    public native int nativeCall();

    public Object reflect(String className) throws Exception {
        Class<?> type = Class.forName(className);
        Method method = type.getDeclaredMethod("toString");
        return method.invoke(this);
    }

    public Class<?> loadWith(ClassLoader loader, String className) throws ClassNotFoundException {
        return loader.loadClass(className);
    }

    public Object unsafeRead(Unsafe unsafe, Object target, long offset) {
        return unsafe.getObject(target, offset);
    }

    public void loadLibrary() {
        System.loadLibrary("ferrum_fake");
    }

    public Runnable lambda() {
        return () -> System.out.println("lambda");
    }

    public Object proxy(ClassLoader loader) {
        return Proxy.newProxyInstance(
            loader,
            new Class<?>[] { Runnable.class },
            (proxy, method, args) -> null
        );
    }

    public MethodHandle invokeApi() throws NoSuchMethodException, IllegalAccessException {
        return MethodHandles.lookup().findVirtual(
            String.class,
            "length",
            MethodType.methodType(int.class)
        );
    }

    public Class<?> runtimeBytecodeGeneratorMarker() throws ClassNotFoundException {
        return Class.forName("org.objectweb.asm.ClassWriter");
    }

    public int catchOne(String value) {
        try {
            return Integer.parseInt(value);
        } catch (NumberFormatException error) {
            return -1;
        }
    }

    public int arraySum(int[] values) {
        int sum = 0;
        for (int value : values) {
            sum += value;
        }
        return sum;
    }
}
