#!/usr/bin/env python3
"""d2_finit_probe.py — D2 round 2. Read the oracle generator's terminal
V/I (a) at the converged snapshot, (b) right after `Set mode=dynamics`
(InitStateVars/FInit executed, no dynamics Solve yet), (c) after step 1.
Reconstruct the positive-seq E1 = V012[1]-I012[1]*Zsp at each point to see
what current FInit actually consumed. Usage: python d2_finit_probe.py <twin.dll>"""
import os, sys, cmath
from dss import dss

def set_a():
    a = complex(-0.5, 0.866025403); aa = complex(-0.5, -0.866025403)
    m = [0j]*9
    def idx(i,j): return (j-1)*3+(i-1)
    for i in range(1,4):
        m[idx(1,i)] = 1+0j
        if i != 1: m[idx(i,1)] = 1+0j
    m[idx(2,2)] = aa; m[idx(3,3)] = aa
    m[idx(2,3)] = a;  m[idx(3,2)] = a
    return m
def invert(a):
    def idx(i,j): return (j-1)*3+(i-1)
    L=3; lt=[0]*(L+1); t1=0j; k=1
    for _ in range(1,L+1):
        for ll in range(1,L+1):
            if lt[ll]!=1 and abs(a[idx(ll,ll)])-abs(t1) > 0.0:
                t1=a[idx(ll,ll)]; k=ll
        t1=0j; lt[k]=1
        for i in range(1,L+1):
            if i!=k:
                for j in range(1,L+1):
                    if j!=k:
                        a[idx(i,j)] -= (a[idx(i,k)]*a[idx(k,j)])/a[idx(k,k)]
        a[idx(k,k)] = -(1.0/a[idx(k,k)])
        for i in range(1,L+1):
            if i!=k:
                a[idx(i,k)] *= a[idx(k,k)]; a[idx(k,i)] *= a[idx(k,k)]
    for j in range(1,L+1):
        for kk in range(1,L+1):
            a[idx(j,kk)] = -a[idx(j,kk)]
    return a
AP2S = invert(set_a())
def p2s(vph):
    def idx(i,j): return (j-1)*3+(i-1)
    b=[0j]*3
    for i in range(1,4):
        s=0j
        for j in range(1,4):
            s += AP2S[idx(i,j)]*vph[j-1]
        b[i-1]=s
    return b
ZSP = complex(0.2018664, 8.474764893203883)

def vi():
    ckt = dss.ActiveCircuit
    ckt.SetActiveElement("Generator.g1")
    e = ckt.ActiveCktElement
    V = list(e.Voltages); I = list(e.Currents)
    vph = [complex(V[0],V[1]), complex(V[2],V[3]), complex(V[4],V[5])]
    iph = [complex(I[0],I[1]), complex(I[2],I[3]), complex(I[4],I[5])]
    return vph, iph

def report(tag):
    vph, iph = vi()
    v012 = p2s(vph); i012 = p2s(iph)
    E1 = v012[1] - i012[1]*ZSP
    print(f"{tag}: |V1|={abs(v012[1]):.6f} |I1|={abs(i012[1]):.8f} "
          f"I1=({i012[1].real:.6f},{i012[1].imag:.6f}) "
          f"E1=({E1.real:.6f},{E1.imag:.6f}) |E1|={abs(E1):.6f}")

def main():
    twin = os.path.abspath(sys.argv[1]).replace("\\","/")
    here=os.path.dirname(os.path.abspath(__file__))
    root=os.path.abspath(os.path.join(here,"..",".."))
    deck=os.environ.get("DECK", os.path.join(root,"tools","golden","wasm_decks","wasm_gen_dyn.dss"))
    text=open(deck).read().replace("@FIXTURE@",twin)
    dss.Text.Command="clear"
    lines=[l.strip() for l in text.splitlines() if l.strip() and not l.strip().startswith(("!","//"))]
    for cmd in lines:
        if cmd.lower().startswith("solve"): break
        dss.Text.Command=cmd
    dss.Text.Command="Solve"
    report("snapshot          ")
    h=os.environ.get("H","1e-9")
    dss.Text.Command=f"Set mode=dynamics number=1 h={h}"
    report("after set-dyn(FInit)")
    dss.Text.Command="Solve"
    report("step1             ")

if __name__=="__main__":
    main()
