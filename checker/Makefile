TARGET := checker
SRC := checker.cc

all: $(TARGET)

$(TARGET): $(SRC)
	g++ -O2 -static -o $@ $<

clean:
	rm -f $(TARGET)
