#include "testlib.h"

struct Input {
    int L, M, N;
    struct Ops {
        int addr, size, start, tim;
    };
    std::vector<Ops> ops; 
}input;

struct Output {
    struct Ops {
        std::string opName;
        uint64_t T;
        int A, S;
    };
    std::vector<Ops> ops;
}output;

void check_input()
{
    input.L = inf.readInt(1, 100000, "L");
    input.M = inf.readInt(1, input.L, "M");
    input.N = inf.readInt(1, 10000, "N");
    int last_start = 0;
    int N = input.N, L = input.L;
    for (int i = 0; i < N; i++) {
        int addr = inf.readInt(0, L - 1, "addr_i");
        int size = inf.readInt(1, L, "size_i");
        int start = inf.readInt(0, 1e9, "start_i");
        int tim = inf.readInt(0, 1e9, "time_i");
        quitif(addr + size > L, _fail, "[Invalid Input] addr + size > L, where addr = %d, size = %d, L = %d", addr, size, L);
        quitif(start < last_start, _fail, "[Invalid Input] start[%d] > start[%d] (%d > %d)", i - 1, i, last_start, start);
        last_start = start;
        input.ops.push_back({addr, size, start, tim});
    }
    if (inf.seekEof() == false) {
        quitf(_fail, "[Invalid Input] Find task numbers greater than N (which is %d).", N);
    }
    inf.readEof();
}

void check_output() 
{
    int last_visit = -1;
    uint64_t last_T = 0;
    int totalReadLines = 0;
    bool finish = false;
    std::vector<bool> duplicate(input.N, false);

    while (!finish) {
        std::string opName = ouf.readWord();
        uint64_t T;
        int A, S = 0;
        totalReadLines++;
        if (totalReadLines > 10 * input.N) {
            quitf(_pe, "[Invalid Output] Too many lines. %d > 10n", totalReadLines);
        }
        if (opName == "Reload") {
            T = ouf.readLong(0, (long long)1e18, "T");
            A = ouf.readInt(0, input.L - 1, "A");
            S = ouf.readInt(1, input.L, "S");
            quitif(A + S > input.L, _wa, "[Invalid Output] Reload addr + size > L, where addr = %d, size = %d, L = %d", A, S, input.L);
        } else if (opName == "Visit") {
            T = ouf.readLong(0, (long long)1e18, "T");
            A = ouf.readInt(0, input.N - 1, "A");
            quitif(duplicate[A], _wa, "[Invalid Output] Output (Visit %d) more than once.", A);
            duplicate[A] = true;
            if (last_visit != -1 && input.ops[last_visit].start == input.ops[A].start) {
                // PASS. We do not care the Visit order in the same task.
            } else {
                quitif(last_visit >= A, _wa, "[Invalid Output] Tasks are not finished by input sequence. %d >= %d. Op[%d] = (%s %llu %d)",
                    last_visit, A, totalReadLines - 1, opName.c_str(), T, A);
            }
            last_visit = A;
        } else if (opName == "Offload") {
            T = ouf.readLong(0, (long long)1e18, "T");
            A = ouf.readInt(0, input.L - 1, "A");
            S = ouf.readInt(1, input.L, "S");
            quitif(A + S > input.L, _wa, "[Invalid Output] Offload addr + size > L, where addr = %d, size = %d, L = %d.", A, S, input.L);
        } else if (opName == "Fin") {
            T = ouf.readLong(0, (long long)1e18, "T");
            finish = true;
        } else {
            quitf(_wa, "[Invalid Output] Unknown operator %s", opName.c_str());
        }
        quitif(last_T > T, _wa, "[Invalid Output] Output T is not ascending. %llu > %llu. Op[%d] = (%s %llu ...)", 
            last_T, T, totalReadLines - 1, opName.c_str(), T);
        last_T = T;
        output.ops.push_back({opName, T, A, S});
    }
    int finish_tasks = 0;
    for (int i = 0; i < input.N; i++) {
        if (duplicate[i]) {
            finish_tasks++;
        }
    }
    quitif(finish_tasks != input.N, _wa, "[Invalid Output] Output did not finish all Visit tasks. %d != N (which is %d)", finish_tasks, input.N);
    if (ouf.seekEof() == false) {
        quitf(_pe, "[Invalid Output] Find extra lines after output Fin.");
    }
    ouf.readEof();
}

uint64_t get_score() 
{
    const int MEM_OFFLOAD = -2;
    const int MEM_RELOAD = -1;
    const uint64_t multiple_IO = 40;
    std::vector<int> in_mem(input.L, MEM_OFFLOAD);
    std::vector<uint64_t> task_finish_at(input.N, 0);
    int use_mem = 0;
    int nr_output = output.ops.size();
    int last_visit = -1;
    uint64_t score = 0;
    uint64_t io_time = 0, npu_time = 0;
    for (int i = 0; i < nr_output; i++) {
        auto curr = output.ops[i];
        if (curr.opName == "Reload") {
            quitif(io_time > curr.T, _wa, "[Invalid Output] IO is busy. Last IO task finish at %llu. Op[%d] = (%s %llu %d %d)",
                 io_time, i, curr.opName.c_str(), curr.T, curr.A, curr.S);
            int cnt_tomem = 0;
            for (int j = curr.A; j < curr.A + curr.S; j++) {
                if (in_mem[j] == MEM_OFFLOAD) {
                    cnt_tomem++;
                    in_mem[j] = MEM_RELOAD;
                }
            }
            use_mem += cnt_tomem;
            quitif(use_mem > input.M, _wa, "[Invalid Output] Out of Memory. use_mem = %d, M = %d. Op[%d] = (%s %llu %d %d)",
                 use_mem, input.M, i, curr.opName.c_str(), curr.T, curr.A, curr.S);
            io_time = curr.T + multiple_IO * cnt_tomem;
        } else if (curr.opName == "Visit") {
            uint64_t this_task_finish_time = curr.T + input.ops[curr.A].tim;
            if (last_visit != -1 && input.ops[last_visit].start == input.ops[curr.A].start) {
                // PASS. The same task.
                npu_time = std::max(npu_time, this_task_finish_time);
            } else {
                quitif(npu_time > curr.T, _wa, "[Invalid Output] NPU is busy. Last NPU task finish at %llu. Op[%d] = (%s %llu %d)",
                    npu_time, i, curr.opName.c_str(), curr.T, curr.A);
                npu_time = this_task_finish_time;
            }
            task_finish_at[curr.A] = this_task_finish_time;
            last_visit = curr.A;
            quitif(curr.T < input.ops[curr.A].start, _wa, "[Invalid Output] Task %d is not ready. Start time = %llu. Op[%d] = (%s %llu %d)",
                 curr.A, input.ops[curr.A].start, i, curr.opName.c_str(), curr.T, curr.A);
            for (int j = input.ops[curr.A].addr; j < input.ops[curr.A].addr + input.ops[curr.A].size; j++) {
                quitif(in_mem[j] == MEM_OFFLOAD, _wa, "[Invalid Output] Addr %d is not in memory. Op[%d] = (%s %llu %d)",
                    j, i, curr.opName.c_str(), curr.T, curr.A);
                if (in_mem[j] == MEM_RELOAD || task_finish_at[in_mem[j]] < task_finish_at[curr.A]) {
                    in_mem[j] = curr.A;
                }
            } 
        } else if (curr.opName == "Offload") {
            quitif(io_time > curr.T, _wa, "[Invalid Output] IO is busy. Last IO task finish at %llu. Op[%d] = (%s %llu %d %d)",
                 io_time, i, curr.opName.c_str(), curr.T, curr.A, curr.S);
            int cnt_offmem = 0;
            for (int j = curr.A; j < curr.A + curr.S; j++) {
                if (in_mem[j] == MEM_OFFLOAD) {
                    continue;
                }
                if (in_mem[j] != MEM_RELOAD) {
                    quitif(curr.T < task_finish_at[in_mem[j]], _wa, "[Invalid Output] Addr %d is used by NPU task %d at T=%llu. Op[%d] = (%s %llu %d %d)",
                        j, in_mem[j], curr.T, i, curr.opName.c_str(), curr.T, curr.A, curr.S);
                }
                cnt_offmem++;
                in_mem[j] = MEM_OFFLOAD;
            }
            use_mem -= cnt_offmem;
            io_time = curr.T + multiple_IO * cnt_offmem;
        } else if (curr.opName == "Fin") {
            score = std::max(io_time, npu_time);
            quitif(score > curr.T, _wa, "[Invalid Output] Output Fin, but not all the resources are freed. Last IO task finish at %llu, Last NPU task finish at %llu",
                 io_time, npu_time);
            score = curr.T;
        }
    }
    return score;
}

int main(int argc, char *argv[]) 
{
    setName("Global_Memory_Planning_for_LLM");
    registerTestlibCmd(argc, argv);
    check_input();
    check_output();
    auto score = get_score();
    quitf(_ok, "All tasks finish at %llu", score);
}
