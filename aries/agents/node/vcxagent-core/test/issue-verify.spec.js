/* eslint-env jest */
require('jest')
const { createPairedAliceAndFaber } = require('./utils/utils')
const {
  IssuerStateType,
  HolderStateType,
  ProverStateType,
  VerifierStateType,
  ProofVerificationStatus
} = require('@hyperledger/node-vcx-wrapper')
const sleep = require('sleep-promise')
const { initRustLogger } = require('../src')
const { proofRequestDataStandard, proofRequestDataSelfAttest } = require('./utils/data')
const mkdirp = require('mkdirp')

const TAILS_DIR = '/tmp/faber/tails'

jest.setTimeout(1000 * 60 * 4)

beforeAll(async () => {
  initRustLogger(process.env.RUST_LOG || 'vcx=error')
  mkdirp(TAILS_DIR)
})

afterAll(async () => {
  await sleep(500)
})

describe('test update state', () => {
  it('Faber should issue credential, verify proof', async () => {
    const { alice, faber } = await createPairedAliceAndFaber()
    const issuerDid = faber.getFaberDid()
    await faber.buildLedgerPrimitives({ tailsDir: TAILS_DIR, maxCreds: 5 })
    await faber.rotateRevReg(TAILS_DIR, 5)
    await faber.sendCredentialOffer()
    await alice.acceptCredentialOffer()

    await faber.updateStateCredential(IssuerStateType.RequestReceived)
    await faber.sendCredential()
    await alice.updateStateCredential(HolderStateType.Finished)
    await faber.receiveCredentialAck()

    const request = await faber.requestProofFromAlice(proofRequestDataStandard(issuerDid))
    await alice.sendHolderProof(JSON.parse(request), revRegId => TAILS_DIR, { attr_nickname: 'Smith' })
    await faber.updateStateVerifierProof(VerifierStateType.Finished)
    await alice.updateStateHolderProof(ProverStateType.Finished)
    const {
      presentationVerificationStatus,
      presentationAttachment,
      presentationRequestAttachment
    } = await faber.getPresentationInfo()
    expect(presentationVerificationStatus).toBe(ProofVerificationStatus.Valid)
    expect(presentationRequestAttachment.requested_attributes).toStrictEqual({
      attr_basic_identity: {
        names: [
          'name',
          'last_name',
          'sex'
        ],
        restrictions: {
          '$or': [
            {
              issuer_did: 'V4SGRU86Z58d6TV7PBUe6f'
            }
          ]
        }
      },
      attr_date: {
        name: 'date',
        restrictions: {
          issuer_did: 'V4SGRU86Z58d6TV7PBUe6f'
        }
      },
      attr_education: {
        name: 'degree',
        restrictions: {
          'attr::degree::value': 'maths'
        }
      },
      attr_nickname: {
        name: 'nickname',
        self_attest_allowed: true
      }
    })
    expect(presentationAttachment.requested_proof).toStrictEqual({
      revealed_attrs: {
        attr_date: {
          sub_proof_index: 0,
          raw: '05-2018',
          encoded: '101085817956371643310471822530712840836446570298192279302750234554843339322886'
        },
        attr_education: {
          sub_proof_index: 0,
          raw: 'maths',
          encoded: '78137204873448776862705240258723141940757006710839733585634143215803847410018'
        }
      },
      revealed_attr_groups: {
        attr_basic_identity: {
          sub_proof_index: 0,
          values: {
            sex: {
              raw: 'female',
              encoded: '71957174156108022857985543806816820198680233386048843176560473245156249119752'
            },
            name: {
              raw: 'alice',
              encoded: '19831138297880367962895005496563562590284654704047651305948751287370224856720'
            },
            last_name: {
              raw: 'clark',
              encoded: '51192516729287562420368242940555165528396706187345387515033121164720912081028'
            }
          }
        }
      },
      self_attested_attrs: {
        attr_nickname: 'Smith'
      },
      unrevealed_attrs: {},
      predicates: {
        predicate_is_adult: {
          sub_proof_index: 0
        }
      }
    })
  })

  it('Faber should issue credential, revoke credential, verify proof', async () => {
    const { alice, faber } = await createPairedAliceAndFaber()
    const issuerDid = faber.getFaberDid()
    await faber.buildLedgerPrimitives({ tailsDir: TAILS_DIR, maxCreds: 5 })
    await faber.sendCredentialOffer()
    await alice.acceptCredentialOffer()

    await faber.updateStateCredential(IssuerStateType.RequestReceived)
    await faber.sendCredential()
    await alice.updateStateCredential(HolderStateType.Finished)
    await faber.receiveCredentialAck()
    await faber.revokeCredential()

    const request = await faber.requestProofFromAlice(proofRequestDataStandard(issuerDid))
    await alice.sendHolderProof(JSON.parse(request), revRegId => TAILS_DIR, { attr_nickname: 'Smith' })
    await faber.updateStateVerifierProof(VerifierStateType.Finished)
    await alice.updateStateHolderProof(ProverStateType.Failed)
    const {
      presentationVerificationStatus
    } = await faber.getPresentationInfo()
    expect(presentationVerificationStatus).toBe(ProofVerificationStatus.Invalid)
  })

  it('Faber should verify proof with self attestation', async () => {
    const { alice, faber } = await createPairedAliceAndFaber()
    const request = await faber.requestProofFromAlice(proofRequestDataSelfAttest())
    await alice.sendHolderProofSelfAttested(JSON.parse(request), { attr_nickname: 'Smith' })
    await faber.updateStateVerifierProof(VerifierStateType.Finished)
    await alice.updateStateHolderProof(ProverStateType.Finished)
    const {
      presentationVerificationStatus,
      presentationAttachment,
      presentationRequestAttachment
    } = await faber.getPresentationInfo()
    expect(presentationVerificationStatus).toBe(ProofVerificationStatus.Valid)
    expect(presentationAttachment.requested_proof).toStrictEqual({
      revealed_attrs: {},
      self_attested_attrs: {
        attr_nickname: 'Smith'
      },
      unrevealed_attrs: {},
      predicates: {}
    })
    expect(presentationRequestAttachment.requested_attributes).toStrictEqual({
      attr_nickname: {
        name: 'nickname',
        self_attest_allowed: true
      }
    })
  })
})
