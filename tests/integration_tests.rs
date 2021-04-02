use as_witx_lib::Generator;
use witx::*;
use std::{fs::File, io::Read, path::PathBuf};

use rayon::prelude::*;
use dissimilar::{diff, Chunk};

#[test]
fn main() -> anyhow::Result<()> {
    let base: PathBuf = std::env::var("CARGO_MANIFEST_DIR").unwrap().into();
    let test_in_dir = base.join("tests/input");
    let test_out_dir = base.join("tests/output");
    let tests = std::fs::read_dir(&test_in_dir)?
        .filter_map(|f| {
            f.ok().and_then(|f| {
                let ftype = f.file_type().ok()?;
                if ftype.is_dir() {
                    return None;
                }
                let os_name =  f.file_name();
                let name = os_name.to_string_lossy().to_owned();
                println!("{:?}", test_in_dir.clone().join(name.as_ref()));
                Some((test_in_dir.clone().join(name.as_ref())
                    , test_out_dir.clone().join(to_ts(&name))))
            })
        })
        .collect::<Vec<_>>();
    
    let results = tests.into_par_iter()
        .map(|test| {
            run_test(test)
        })
        .collect::<Vec<_>>();

    for result in results {
        let (diff, report) = result?;
        assert!(diff, "\n----- Diff Report -----\n{}", report);
    }

    Ok(())
}

fn to_ts(name: &str) -> String {
    let base = name.split('.').next().unwrap();
    format!("{}.ts", base)
}

fn run_test((in_path, out_path): (PathBuf, PathBuf)) -> anyhow::Result<(bool, String)> {
    let gen = Generator::new(None, false)
        .generate(in_path)?;

    let mut f = File::open(&out_path)
        .map_err(|_| anyhow::anyhow!("failed to open {:?}", &out_path))?;
    let mut expected = String::new();
    let _ = f.read_to_string(&mut expected)?;

    let diffs = Diff(diff(&gen, &expected));
    Ok((diffs.is_equal(), format!("{}", diffs)))
}

use std::fmt;

pub struct Diff<'a>(Vec<Chunk<'a>>);

impl<'a> fmt::Display for Diff<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in &self.0 {
            match c {
                Chunk::Equal(x) => {
                    write!(f, "{}", x)?
                },
                Chunk::Insert(x) => {
                    write!(f, "\x1b[92m{}\x1b[0m", x.replace('\n', "\\n"))?
                },
                Chunk::Delete(x) => {
                    write!(f, "\x1b[91m{}\x1b[0m", x.replace('\n', "\\n"))?
                }
            }
        }
        Ok(())
    }
}

impl Diff<'_> {
    fn is_equal(&self) -> bool {
        !self.0.iter().any(|c| match c {
            Chunk::Delete(_) | Chunk::Insert(_) => true,
            Chunk::Equal(_) => false,
        })
    }

    fn diff_chunks<'a>(&'a self) -> impl Iterator<Item=&'a Chunk<'a>> {
        self.0.iter().filter(|c| match c {
            Chunk::Delete(_) | Chunk::Insert(_) => true,
            Chunk::Equal(_) => false,
        })
    }
}

/*
#  Returns a minimal list of differences between 2 lists e and f
#  requring O(min(len(e),len(f))) space and O(min(len(e),len(f)) * D)
#  worst-case execution time where D is the number of differences.
def diff(e, f, i=0, j=0):
  #  Documented at http://blog.robertelder.org/diff-algorithm/
  N,M,L,Z = len(e),len(f),len(e)+len(f),2*min(len(e),len(f))+2
  if N > 0 and M > 0:
    w,g,p = N-M,[0]*Z,[0]*Z
    for h in range(0, (L//2+(L%2!=0))+1):
      for r in range(0, 2):
        c,d,o,m = (g,p,1,1) if r==0 else (p,g,0,-1)
        for k in range(-(h-2*max(0,h-M)), h-2*max(0,h-N)+1, 2):
          a = c[(k+1)%Z] if (k==-h or k!=h and c[(k-1)%Z]<c[(k+1)%Z]) else c[(k-1)%Z]+1
          b = a-k
          s,t = a,b
          while a<N and b<M and e[(1-o)*N+m*a+(o-1)]==f[(1-o)*M+m*b+(o-1)]:
            a,b = a+1,b+1
          c[k%Z],z=a,-(k-w)
          if L%2==o and z>=-(h-o) and z<=h-o and c[k%Z]+d[z%Z] >= N:
            D,x,y,u,v = (2*h-1,s,t,a,b) if o==1 else (2*h,N-a,M-b,N-s,M-t)
            if D > 1 or (x != u and y != v):
              return diff(e[0:x],f[0:y],i,j)+diff(e[u:N],f[v:M],i+u,j+v)
            elif M > N:
              return diff([],f[N:M],i+N,j+N)
            elif M < N:
              return diff(e[M:N],[],i+M,j+M)
            else:
              return []
  elif N > 0: #  Modify the return statements below if you want a different edit script format
    return [{"operation": "delete", "position_old": i+n} for n in range(0,N)]
  else:
    return [{"operation": "insert", "position_old": i,"position_new":j+n} for n in range(0,M)]
 */