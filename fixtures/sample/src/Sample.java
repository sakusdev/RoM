package example;

import java.util.List;

public final class Sample implements Runnable {
    private final String name;
    public static final int ANSWER = 42;

    public Sample(String name) {
        this.name = name;
    }

    public String greeting(int count) {
        return name + ":" + count;
    }

    public List<String> values() {
        return List.of(name, greeting(ANSWER));
    }

    @Override
    public void run() {
        System.out.println(greeting(1));
    }

    public enum Mode {
        FAST,
        SAFE
    }
}
